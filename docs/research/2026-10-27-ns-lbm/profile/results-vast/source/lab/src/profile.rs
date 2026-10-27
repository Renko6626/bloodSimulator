//! Opt-in diagnostics only; no solver state or stepping decisions live here.
use std::cell::RefCell;
use std::ffi::{c_char, c_int, c_void};
use std::sync::OnceLock;
use std::time::Instant;
#[repr(C)]
struct Timespec {
    sec: i64,
    nsec: i64,
}
unsafe extern "C" {
    fn clock_gettime(clock: c_int, value: *mut Timespec) -> c_int;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
}
fn cpu(clock: c_int) -> f64 {
    let mut ts = Timespec { sec: 0, nsec: 0 };
    assert_eq!(unsafe { clock_gettime(clock, &mut ts) }, 0);
    ts.sec as f64 * 1e3 + ts.nsec as f64 * 1e-6
}
fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NS_PROFILE").is_some())
}
fn mark(stage: u32) {
    type Mark = unsafe extern "C" fn(u32);
    static MARK: OnceLock<Option<Mark>> = OnceLock::new();
    let f = MARK.get_or_init(|| unsafe {
        let ptr = dlsym(std::ptr::null_mut(), c"ns_prof_stage".as_ptr());
        if ptr.is_null() {
            None
        } else {
            Some(std::mem::transmute::<*mut c_void, Mark>(ptr))
        }
    });
    if let Some(f) = f {
        unsafe { f(stage) }
    }
}
struct Row {
    frame: usize,
    stage: &'static str,
    wall: f64,
    process: f64,
    main: f64,
}
thread_local! {static ROWS:RefCell<Vec<Row>>=const{RefCell::new(Vec::new())}; static FRAME:std::cell::Cell<usize>=const{std::cell::Cell::new(usize::MAX)};}
pub fn frame(index: Option<usize>) {
    if enabled() {
        FRAME.with(|x| x.set(index.unwrap_or(usize::MAX)));
    }
}
pub struct Span {
    start: Option<(Instant, f64, f64)>,
    stage: &'static str,
}
pub fn span(stage: &'static str, id: u32) -> Span {
    let active = enabled() && FRAME.with(|x| x.get() != usize::MAX);
    if active {
        mark(id);
        Span {
            start: Some((Instant::now(), cpu(2), cpu(3))),
            stage,
        }
    } else {
        Span { start: None, stage }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some((wall, process, main)) = self.start {
            let process = cpu(2) - process;
            let main = cpu(3) - main;
            let wall = wall.elapsed().as_secs_f64() * 1e3;
            mark(0);
            ROWS.with(|r| {
                r.borrow_mut().push(Row {
                    frame: FRAME.with(|x| x.get()),
                    stage: self.stage,
                    wall,
                    process,
                    main,
                })
            });
        }
    }
}
pub fn write(prefix: &std::path::Path) {
    if !enabled() {
        return;
    }
    use std::io::Write;
    let mut f = std::io::BufWriter::new(
        std::fs::File::create(prefix.with_extension("stages.csv")).unwrap(),
    );
    writeln!(f, "frame,stage,wall_ms,process_cpu_ms,main_cpu_ms").unwrap();
    ROWS.with(|r| {
        for x in r.borrow().iter() {
            writeln!(
                f,
                "{},{},{:.9},{:.9},{:.9}",
                x.frame, x.stage, x.wall, x.process, x.main
            )
            .unwrap();
        }
    });
}

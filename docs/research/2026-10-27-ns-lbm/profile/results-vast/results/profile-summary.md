# Vast NS profile 原始汇总

| 网格 | 观测组 | 完整 mean / p95 ms | 流体+耦合 mean / p95 ms |
| --- | --- | ---: | ---: |
| 192² | baseline | 68.634 / 72.609 | 64.324 / 68.136 |
| 192² | detail1 | 177.029 / 180.337 | 167.933 / 170.870 |
| 192² | detail4 | 68.048 / 72.664 | 63.683 / 68.128 |
| 192² | off1 | 177.370 / 180.277 | 168.265 / 171.008 |
| 192² | off4 | 68.986 / 73.379 | 64.605 / 68.827 |
| 256² | baseline | 118.431 / 133.638 | 113.523 / 128.385 |
| 256² | detail1 | 269.218 / 283.616 | 261.621 / 275.409 |
| 256² | detail4 | 126.943 / 135.366 | 121.696 / 129.404 |
| 256² | off1 | 269.371 / 281.805 | 261.692 / 273.799 |
| 256² | off4 | 122.535 / 134.647 | 117.428 / 129.000 |

off=观测关闭；detail=分段 CPU／墙钟计时；数字为 Rayon 工作线程数。baseline 为首批原始二进制。每组两轮，共 240 测量帧。

| 网格 | 组 | 阶段 | mean wall ms/frame | mean process CPU ms/frame | mean main CPU ms/frame |
| --- | --- | --- | ---: | ---: | ---: |
| 192² | detail1 | advection | 90.701 | 90.685 | 0.114 |
| 192² | detail1 | coupling | 0.423 | 0.415 | 0.416 |
| 192² | detail1 | diagnostics | 1.451 | 1.447 | 1.448 |
| 192² | detail1 | diffusion_u | 24.950 | 24.935 | 24.937 |
| 192² | detail1 | diffusion_v | 24.226 | 24.213 | 24.215 |
| 192² | detail1 | drive | 0.084 | 0.077 | 0.076 |
| 192² | detail1 | pressure_correct | 1.486 | 1.478 | 1.479 |
| 192² | detail1 | pressure_rhs | 2.713 | 2.703 | 2.705 |
| 192² | detail1 | pressure_solve | 23.342 | 23.331 | 23.332 |
| 192² | detail1 | rapier | 7.638 | 7.968 | 6.678 |
| 192² | detail4 | advection | 19.926 | 78.822 | 0.040 |
| 192² | detail4 | coupling | 0.243 | 0.240 | 0.239 |
| 192² | detail4 | diagnostics | 0.805 | 0.806 | 0.803 |
| 192² | detail4 | diffusion_u | 13.926 | 14.165 | 13.916 |
| 192² | detail4 | diffusion_v | 13.498 | 13.502 | 13.489 |
| 192² | detail4 | drive | 0.057 | 0.191 | 0.053 |
| 192² | detail4 | pressure_correct | 0.931 | 0.927 | 0.927 |
| 192² | detail4 | pressure_rhs | 1.464 | 1.458 | 1.459 |
| 192² | detail4 | pressure_solve | 13.633 | 13.632 | 13.625 |
| 192² | detail4 | rapier | 3.556 | 11.925 | 2.999 |
| 256² | detail1 | advection | 138.540 | 138.501 | 0.113 |
| 256² | detail1 | coupling | 0.320 | 0.314 | 0.315 |
| 256² | detail1 | diagnostics | 1.869 | 1.866 | 1.866 |
| 256² | detail1 | diffusion_u | 42.678 | 42.661 | 42.663 |
| 256² | detail1 | diffusion_v | 40.713 | 40.701 | 40.703 |
| 256² | detail1 | drive | 0.133 | 0.126 | 0.127 |
| 256² | detail1 | pressure_correct | 1.836 | 1.830 | 1.832 |
| 256² | detail1 | pressure_rhs | 3.778 | 3.770 | 3.772 |
| 256² | detail1 | pressure_solve | 33.613 | 33.601 | 33.602 |
| 256² | detail1 | rapier | 5.724 | 5.995 | 4.867 |
| 256² | detail4 | advection | 35.015 | 138.445 | 0.041 |
| 256² | detail4 | coupling | 0.251 | 0.249 | 0.247 |
| 256² | detail4 | diagnostics | 1.408 | 1.436 | 1.406 |
| 256² | detail4 | diffusion_u | 27.729 | 27.909 | 27.713 |
| 256² | detail4 | diffusion_v | 26.845 | 26.850 | 26.834 |
| 256² | detail4 | drive | 0.085 | 0.274 | 0.081 |
| 256² | detail4 | pressure_correct | 1.657 | 1.652 | 1.653 |
| 256² | detail4 | pressure_rhs | 2.617 | 2.611 | 2.612 |
| 256² | detail4 | pressure_solve | 27.491 | 27.504 | 27.483 |
| 256² | detail4 | rapier | 3.834 | 12.521 | 3.220 |

## sample-192-t4
{"records": 16541, "stored": 16541, "overruns_in_active_stages": 2, "timer_failures": 0, "timers": 5, "interval_ns": 2004008, "main_tid": 1150, "format": "little-endian u64 PC,u32 tid,u32 stage"}

### advection
- 6646 (69.6%): ns_mac_lab::grid::weights
- 1171 (12.3%): <&<ns_mac_lab::grid::Grid>::advect::{closure#0} as core::ops::function::FnMut<((usize, &mut f64),)>>::call_mut
- 855 (9.0%): _RNvNtCsfkm9AioTFUP_10ns_mac_lab4grid13vector_sample.llvm.17788952620215612864
- 728 (7.6%): floor
- 73 (0.8%): __sched_yield
- 12 (0.1%): syscall
- 11 (0.1%): crossbeam_epoch::default::with_handle::<crossbeam_epoch::default::pin::{closure#0}, crossbeam_epoch::guard::Guard>
- 11 (0.1%): _RNCINvNtCs5ASfKDLXyfD_10rayon_core4join12join_contextNCINvNvNtNtCsi2y6i1rReQX_5rayon4iter8plumbing24bridge_producer_consumer6helperINtNtBY_9enumerate17EnumerateProducerINtNtB10_5slice15IterMutProducerdEEINtNtBY_8for_each15ForEachConsumerNCNvMs0_NtCsfkm9AioTFUP_10ns_mac_lab4gridNtB3V_4Grid6advect0EE0NCBR_s_0uuE0B3X_.llvm.16107912906716103672
### diffusion_u
- 367 (21.0%): <pulp::x86::v3::V3>::vectorize::imp::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::matmul::matvec_colmajor::matvec::Impl<f64>>>
- 323 (18.5%): <pulp::x86::v3::V3>::vectorize::imp::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::matmul::matvec_rowmajor::matvec::Impl<f64>>>
- 288 (16.5%): <faer::sparse::linalg::cholesky::supernodal::SupernodalLltRef<usize, f64>>::solve_in_place_with_conj
- 200 (11.5%): <pulp::x86::v3::V3>::vectorize::imp_fastcall::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::triangular_solve::solve_lower_triangular_in_place_imp<f64>::{closure#2}>>
- 132 (7.6%): faer::linalg::matmul::matmul_imp::<f64>
- 101 (5.8%): faer::linalg::triangular_solve::solve_lower_triangular_in_place_imp::<f64>
- 87 (5.0%): faer::linalg::matmul::matvec_colmajor::matvec::<f64>
- 76 (4.4%): <faer::sparse::linalg::cholesky::supernodal::SupernodalLltRef<usize, f64>>::supernode
### diffusion_v
- 316 (18.9%): <pulp::x86::v3::V3>::vectorize::imp::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::matmul::matvec_rowmajor::matvec::Impl<f64>>>
- 305 (18.3%): <faer::sparse::linalg::cholesky::supernodal::SupernodalLltRef<usize, f64>>::solve_in_place_with_conj
- 299 (17.9%): <pulp::x86::v3::V3>::vectorize::imp::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::matmul::matvec_colmajor::matvec::Impl<f64>>>
- 197 (11.8%): <pulp::x86::v3::V3>::vectorize::imp_fastcall::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::triangular_solve::solve_lower_triangular_in_place_imp<f64>::{closure#2}>>
- 137 (8.2%): faer::linalg::matmul::matmul_imp::<f64>
- 83 (5.0%): faer::linalg::triangular_solve::solve_lower_triangular_in_place_imp::<f64>
- 82 (4.9%): faer::linalg::matmul::matvec_colmajor::matvec::<f64>
- 76 (4.6%): <faer::sparse::linalg::cholesky::supernodal::SupernodalLltRef<usize, f64>>::supernode
### pressure_solve
- 355 (21.0%): <pulp::x86::v3::V3>::vectorize::imp::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::matmul::matvec_colmajor::matvec::Impl<f64>>>
- 340 (20.1%): <pulp::x86::v3::V3>::vectorize::imp::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::matmul::matvec_rowmajor::matvec::Impl<f64>>>
- 301 (17.8%): <faer::sparse::linalg::cholesky::supernodal::SupernodalLltRef<usize, f64>>::solve_in_place_with_conj
- 175 (10.3%): <pulp::x86::v3::V3>::vectorize::imp_fastcall::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::triangular_solve::solve_lower_triangular_in_place_imp<f64>::{closure#2}>>
- 134 (7.9%): faer::linalg::matmul::matmul_imp::<f64>
- 83 (4.9%): faer::linalg::triangular_solve::solve_lower_triangular_in_place_imp::<f64>
- 81 (4.8%): faer::linalg::matmul::matvec_colmajor::matvec::<f64>
- 63 (3.7%): faer::linalg::matmul::matvec_rowmajor::matvec::<f64>
### rapier
- 443 (31.3%): rapier2d::dynamics::solver::staged_island_solver::solve::solve_pass
- 350 (24.7%): <rapier2d::dynamics::solver::soft_constraint::soft_constraints_set::soft_constraints_set::SoftConstraintsSet>::solve_color_constraint
- 110 (7.8%): __sched_yield
- 81 (5.7%): <rapier2d::dynamics::solver::soft_constraint::soft_element_constraint::soft_scalar_constraint::SoftScalarConstraint>::apply_impulse
- 55 (3.9%): rapier2d::dynamics::solver::staged_island_solver::worker::run_worker
- 49 (3.5%): <rapier2d::dynamics::solver::soft_constraint::soft_constraints_set::soft_constraints_set::SoftConstraintsSet>::solve_volume_constraint
- 22 (1.6%): syscall
- 21 (1.5%): <rapier2d::dynamics::solver::soft_constraint::soft_constraints_set::soft_constraints_set::SoftConstraintsSet>::prepare_awake_body

## sample-256-t4
{"records": 28060, "stored": 28060, "overruns_in_active_stages": 2, "timer_failures": 0, "timers": 5, "interval_ns": 2004008, "main_tid": 1155, "format": "little-endian u64 PC,u32 tid,u32 stage"}

### advection
- 11700 (71.0%): ns_mac_lab::grid::weights
- 1985 (12.1%): <&<ns_mac_lab::grid::Grid>::advect::{closure#0} as core::ops::function::FnMut<((usize, &mut f64),)>>::call_mut
- 1370 (8.3%): _RNvNtCsfkm9AioTFUP_10ns_mac_lab4grid13vector_sample.llvm.17788952620215612864
- 1270 (7.7%): floor
- 53 (0.3%): __sched_yield
- 26 (0.2%): _RNCINvNtCs5ASfKDLXyfD_10rayon_core4join12join_contextNCINvNvNtNtCsi2y6i1rReQX_5rayon4iter8plumbing24bridge_producer_consumer6helperINtNtBY_9enumerate17EnumerateProducerINtNtB10_5slice15IterMutProducerdEEINtNtBY_8for_each15ForEachConsumerNCNvMs0_NtCsfkm9AioTFUP_10ns_mac_lab4gridNtB3V_4Grid6advect0EE0NCBR_s_0uuE0B3X_.llvm.16107912906716103672
- 21 (0.1%): rayon::iter::plumbing::bridge_producer_consumer::helper::<rayon::iter::enumerate::EnumerateProducer<rayon::slice::IterMutProducer<f64>>, rayon::iter::for_each::ForEachConsumer<<ns_mac_lab::grid::Grid>::advect::{closure#0}>>
- 19 (0.1%): syscall
### diffusion_u
- 854 (26.7%): <pulp::x86::v3::V3>::vectorize::imp::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::matmul::matvec_colmajor::matvec::Impl<f64>>>
- 653 (20.4%): <pulp::x86::v3::V3>::vectorize::imp::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::matmul::matvec_rowmajor::matvec::Impl<f64>>>
- 476 (14.9%): <faer::sparse::linalg::cholesky::supernodal::SupernodalLltRef<usize, f64>>::solve_in_place_with_conj
- 372 (11.6%): <pulp::x86::v3::V3>::vectorize::imp_fastcall::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::triangular_solve::solve_lower_triangular_in_place_imp<f64>::{closure#2}>>
- 216 (6.8%): faer::linalg::matmul::matmul_imp::<f64>
- 131 (4.1%): faer::linalg::triangular_solve::solve_lower_triangular_in_place_imp::<f64>
- 114 (3.6%): <faer::sparse::linalg::cholesky::supernodal::SupernodalLltRef<usize, f64>>::supernode
- 111 (3.5%): faer::linalg::matmul::matvec_rowmajor::matvec::<f64>
### diffusion_v
- 712 (23.2%): <pulp::x86::v3::V3>::vectorize::imp::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::matmul::matvec_colmajor::matvec::Impl<f64>>>
- 613 (20.0%): <pulp::x86::v3::V3>::vectorize::imp::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::matmul::matvec_rowmajor::matvec::Impl<f64>>>
- 446 (14.5%): <faer::sparse::linalg::cholesky::supernodal::SupernodalLltRef<usize, f64>>::solve_in_place_with_conj
- 392 (12.8%): <pulp::x86::v3::V3>::vectorize::imp_fastcall::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::triangular_solve::solve_lower_triangular_in_place_imp<f64>::{closure#2}>>
- 217 (7.1%): faer::linalg::matmul::matmul_imp::<f64>
- 134 (4.4%): faer::linalg::triangular_solve::solve_lower_triangular_in_place_imp::<f64>
- 124 (4.0%): faer::linalg::matmul::matvec_rowmajor::matvec::<f64>
- 123 (4.0%): <faer::sparse::linalg::cholesky::supernodal::SupernodalLltRef<usize, f64>>::supernode
### pressure_solve
- 733 (23.3%): <pulp::x86::v3::V3>::vectorize::imp::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::matmul::matvec_colmajor::matvec::Impl<f64>>>
- 673 (21.4%): <pulp::x86::v3::V3>::vectorize::imp::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::matmul::matvec_rowmajor::matvec::Impl<f64>>>
- 474 (15.1%): <faer::sparse::linalg::cholesky::supernodal::SupernodalLltRef<usize, f64>>::solve_in_place_with_conj
- 381 (12.1%): <pulp::x86::v3::V3>::vectorize::imp_fastcall::<<pulp::x86::v3::V3 as pulp::Simd>::vectorize::Impl<faer::linalg::triangular_solve::solve_lower_triangular_in_place_imp<f64>::{closure#2}>>
- 214 (6.8%): faer::linalg::matmul::matmul_imp::<f64>
- 135 (4.3%): faer::linalg::triangular_solve::solve_lower_triangular_in_place_imp::<f64>
- 130 (4.1%): <faer::sparse::linalg::cholesky::supernodal::SupernodalLltRef<usize, f64>>::supernode
- 129 (4.1%): faer::linalg::matmul::matvec_rowmajor::matvec::<f64>
### rapier
- 406 (27.3%): rapier2d::dynamics::solver::staged_island_solver::solve::solve_pass
- 333 (22.4%): <rapier2d::dynamics::solver::soft_constraint::soft_constraints_set::soft_constraints_set::SoftConstraintsSet>::solve_color_constraint
- 177 (11.9%): __sched_yield
- 83 (5.6%): <rapier2d::dynamics::solver::soft_constraint::soft_element_constraint::soft_scalar_constraint::SoftScalarConstraint>::apply_impulse
- 73 (4.9%): rapier2d::dynamics::solver::staged_island_solver::worker::run_worker
- 53 (3.6%): <rapier2d::dynamics::solver::soft_constraint::soft_constraints_set::soft_constraints_set::SoftConstraintsSet>::solve_volume_constraint
- 35 (2.4%): <rapier2d::dynamics::solver::soft_constraint::soft_constraints_set::soft_constraints_set::SoftConstraintsSet>::writeback_constraints
- 33 (2.2%): syscall

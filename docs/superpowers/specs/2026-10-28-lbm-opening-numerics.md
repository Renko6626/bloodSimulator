# Fixed-flow opening reconstruction

For each boundary cell, let n point inward and t=(-n_y,n_x). The three incoming directions have c·n=1; six other populations are known after streaming. Convert shifted populations g to physical f=g+w. Let A be the sum over c·n=0, B the sum over c·n=-1, and D=f_t-f_-t. With lattice velocity u and lattice force density F, define

    rho = (A + 2B - F_n/2)/(1-u_n)
    k_n = rho*u_n-F_n/2,  k_t = rho*u_t-F_t/2
    f_n = f_-n + 2*k_n/3
    f_(n+t) = f_(-n-t) + k_n/6 + (k_t-D)/2
    f_(n-t) = f_(-n+t) + k_n/6 - (k_t-D)/2

Writing only these three populations yields sum(f)=rho and sum(c*f)=rho*u-F/2. All four sides use the same rotated formula. World normal velocity is integrated as sum(u·outward*h), with graph depth one world unit; the velocity tangent follows the supplied vessel direction. Reconstruction follows streaming and precedes macroscopic readout and the next collision. Other perimeter links and solid links bounce back, and corners are rejected.

Graph rates are circular-volume flow per game time. Use a common equivalent depth D = 1 world unit throughout the window: planar flux q = Q / D. This convention preserves conserved graph rates, but does not reproduce circular mean speed.

For a straight segment of half-width R, tangent t (from graph edge source to destination), lateral distance d and graph rate Q, initialize velocity t * (3 q / (4 R)) * max(0, 1 - (d/R)^2). At openings the discrete center-sampled profile is normalized by its sum times grid spacing and the normal component of t, so the outward integral equals signed q. Opening endpoints are the grid-cell range endpoints on the world boundary. Openings with curved local buffers, ambiguous segments or corners are unsupported; bends away from the local buffer are allowed. The initial profile at junctions uses the nearest edge and is an initial condition, not a divergence-free global solution.

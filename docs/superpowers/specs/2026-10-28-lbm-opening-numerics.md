# LBM opening geometry and flow contract

Graph rates are circular-volume flow per game time. Use one common equivalent depth D = 1 world unit throughout the window: planar flux q = Q / D. The same D applies to every vessel, so conserved graph rates remain conserved even when radii differ. This is a modeling convention, not a claim that the 2D profile reproduces circular mean speed.

For a straight segment of half-width R, tangent t (from graph edge source to destination), lateral distance d and graph rate Q, initialize velocity t * (3 q / (4 R)) * max(0, 1 - (d/R)^2). At openings the discrete center-sampled profile is normalized by its sum times grid spacing and the normal component of t, so the outward integral equals signed q. Opening endpoints are the grid-cell range endpoints on the world boundary. Non-straight, ambiguous or corner openings are unsupported. The initial profile at junctions uses the nearest edge and is an initial condition, not a divergence-free global solution.

GPU incoming-population reconstruction and Guo half-force treatment are the next substep; this document does not claim they are implemented.

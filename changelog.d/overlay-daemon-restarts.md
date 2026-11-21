Added

- **Overlay journeys for daemon restarts.** The overlay-functional harness now restarts tor, i2pd and cjdroute under a running node and checks the node is reachable again at the same address without a restart: `tor_onion_comes_back_after_tor_restarts`, `i2p_destination_comes_back_after_i2pd_restarts` and `cjdns_listener_works_after_cjdroute_restarts`.

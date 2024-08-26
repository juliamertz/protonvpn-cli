# Work in progress

## Features
- [x] Filter by speed, load and exit country
- [x] Connect to random or fastest server from list
- [x] Select protocol, `tcp` or `udp` with -p flag
- [x] Filter by features
- [x] Killswitch (still needs some work on macos)
- [ ] Split tunneling 

## Todo / Bugs
- [ ] Clean up openvpn process if this wasn't done on last exit
- [ ] Set up test suite
- [ ] Alert user in case traffic isn't actually being routed through tunnel
- [ ] Openvpn process not being killed if daemon is shut down before connection is established
- [ ] Add notify feature for desktop notifications

## features

### Killswitch

#### Notes:
**Linux**
> Warning! this alters your iptables, if you're using a non-standard setup make sure the new rules don't conflict

**Macos**
> In order to use the killswitch you have to enable the system filewall, this can be done in the system settings under `Network > Firewall`

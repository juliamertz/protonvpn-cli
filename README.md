# Work in progress

## Features

- [x] Connect to random or fastest server from list
- [x] Filter by features
- [x] Killswitch (still needs some work on macos)
- [ ] Split tunneling

## Todo / Bugs

- [ ] Nix Darwin module
- [ ] Clean up openvpn process if this wasn't done on last exit
- [ ] Set up test suite
- [ ] Alert user in case traffic isn't actually being routed through tunnel
- [ ] Openvpn process not being killed if daemon is shut down before connection is established
- [ ] Add notify feature for desktop notifications
- [ ] Merge default_criteria config option with cli flags

## Features

### Filtering servers

> TODO: Filter by features for cli

There are many flags to filter servers, these can be used with the `connect` and `query` subcommands.
| Flag | Description |
| --------------------------|----------------------------------------------------------------------------|
| `-c`, `--country` | Filter servers by country [possible values: [here](src/api/types.rs#L136)] |
| `-t`, `--tier <tier>` | servers by tier [possible values: free, premium, all] |
| `-s`, `--sort <sort>` | servers [possible values: speed, load] |
| `--max-load <max-load>` | servers by load |
| `--tor` | include servers with the Tor feature |
| `--p2p` | include servers with the P2P feature |
| `--secure-core` | include servers with the Secure Core feature |
| `--streaming` | include servers with the Streaming feature |

### Protocol

You can use either `udp` or `tcp`, change this with the command line flag `--port` or `-p`.
To set a default, change the value of `default_protocol` in your config file.

### Killswitch

Enabling the killswitch will apply some firewall rules that only allow traffic to pass through the openvpn tunnel.
You can enable the killswitch by running `pvpn killswitch enable` or set the `killswitch.enable` config option to `true`

If you require extra firewall rules you can add these under `killswitch.custom_rules`, for example:

```ron
killswitch: (
  enable: true,
  custom_rules: Some([
    "-A INPUT -s 192.168.0.100 -j ACCEPT",
    "-A OUTPUT -d 192.168.0.100 -j ACCEPT",
    "-A INPUT -p tcp -m tcp --dport 22 -j ACCEPT"
  ]),
),
```

#### Notes

**Linux**

> Warning! this alters your iptables, if you're using a non-standard setup make sure the new rules don't conflict

**Macos**

> In order to use the killswitch you have to enable the system filewall, this can be done in the system settings under `Network > Firewall`

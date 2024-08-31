{ lib, config, ... }:
let
  cfg = config.services.protonvpn;

  toStr = x:
    if lib.isBool x then if x then "true" else "false" else builtins.toString x;

  formatPort = port:
    if lib.isAttrs port then
      "${toStr port.from}:${toStr port.to}"
    else
      toStr port;

  generateRule = proto: port:
    map
    (chain: "-A ${chain} -p ${proto} --dport ${formatPort port} -j ACCEPT") [
      "INPUT"
      "OUTPUT"
    ];

  generateRules = proto: ports:
    lib.flatten (map (port: generateRule proto port) ports);

  pad = ident: lib.concatStrings (builtins.genList (_: "  ") ident);
  br = "\n";

  formatSection = n: key: value:
    if builtins.isAttrs value then
      formatStruct n key value
    else
      (pad n) + "${key}: ${toStr value},";

  formatSections = n: attrs:
    lib.concatStringsSep "\n"
    (lib.mapAttrsToList (key: value: formatSection n key value) attrs);

  formatStruct = n: name: attrs:
    "${pad n}${name}: (${br}${formatSections (n + 1) attrs}${br}${pad n}),";

  ron.format = attrs: "(${br}${formatSections 1 attrs}${br})";
  ron.types = {
    str = x: ''"${toStr x}"'';
    option = x: if builtins.isNull x then "None" else "Some(${x})";
    array = list:
      if builtins.isNull list then
        null
      else
        "[${lib.concatStringsSep ", " list}]";
  };

  allRules = with config.networking.firewall;
    lib.flatten [
      (generateRules "udp" (allowedUDPPorts ++ allowedUDPPortRanges))
      (generateRules "tcp" (allowedTCPPorts ++ allowedTCPPortRanges))
    ];

  content = let
    inherit (cfg) settings;
    inherit (ron.types) str option array;
  in ron.format {
    inherit (settings)
      max_cache_age autostart_default default_select default_protocol;
    credentials_path = option (str settings.credentials_path);
    update_resolv_conf_path = option (str settings.update_resolv_conf_path);

    default_criteria = with settings.default_criteria; {
      inherit tier max_load;
      country = option country;
      features = array features;
    };

    killswitch = with settings.killswitch; {
      inherit enable;
      custom_rules = option (array (map str
        (custom_rules ++ (lib.optionals applyFirewallRules allRules))));
    };
  };
in { environment.etc."protonvpn-rs/config.ron".text = content; }

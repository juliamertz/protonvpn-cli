{ protonvpn-rs }:
{ pkgs, config, ... }:
let
  inherit (pkgs) lib;
  cfg = config.services.protonvpn;
  serviceName = "protonvpn-rs";
in {
  options.services.protonvpn = with lib; {
    enable = mkEnableOption (mdDoc "ProtonVPN service");
    requireSops = mkOption {
      type = types.bool;
      default = false;
      description = mkDoc ''
        Make sure the service starts after sops has decypted your secrets.
        This is usefull if you use sops to store your openvpn credentials.
      '';
    };

    settings = {
      max_cache_age = mkOption {
        type = types.number;
        default = 3;
        description = mkDoc ''
          Maximum cache age in days
        '';
      };
      autostart_default = mkOption {
        type = types.bool;
        default = false;
        description = mkDoc ''
          Automatically connect to a server matching the default criteria on startup.
        '';
      };
      credentials_path = mkOption {
        type = types.nullOr types.str;
        default = null;
        description = mkDoc ''
          Path to the openvpn authentication credentials
        '';
      };
      update_resolv_conf_path = mkOption {
        type = types.path;
        default =
          "${pkgs.update-resolv-conf}/libexec/openvpn/update-resolv-conf";
        description = mkDoc ''
          Path to update-resolv-conf script used by openvpn.
        '';
      };
      default_select = mkOption {
        type = types.str;
        default = "Fastest";
        description = mkDoc ''
          Which strategy to use when automatically selecting a server.
          Choice of: [Fastest, Random]
        '';
      };
      default_protocol = mkOption {
        type = types.str;
        default = "Udp";
        description = mkDoc ''
          Which protocol OpenVPN uses
          Choice of: [Udp, Tcp]
        '';
      };

      killswitch = {
        enable = mkEnableOption (mdDoc "ProtonVPN service");
        custom_rules = mkOption {
          type = types.nullOr (types.listOf types.str);
          default = null;
        };
      };

      default_criteria = {
        tier = mkOption {
          type = types.nullOr (types.enum [ "Premium" "Free" ]);
          default = "Premium";
          description = mkDoc ''
            Choice of: [Premium, Free]
          '';
        };
        country = mkOption {
          type = types.nullOr types.str;
          default = null;
          description = mkDoc ''
            Uppercase country code
          '';
        };
        max_load = mkOption {
          type = types.number;
          default = 90;
        };
        features = mkOption {
          type = types.nullOr (types.listOf types.str);
          default = [ "Streaming" ];
        };
      };
    };
  };
  config = {
    environment.systemPackages = [ protonvpn-rs ];

    environment.etc."protonvpn-rs/config.ron".text = let
      toStr = x:
        if lib.isBool x then
          if x then "true" else "false"
        else
          builtins.toString x;
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
    in with cfg.settings;
    with ron.types;
    (ron.format {
      inherit max_cache_age autostart_default default_select default_protocol;
      credentials_path = option (str credentials_path);
      update_resolv_conf_path = option (str update_resolv_conf_path);

      default_criteria = with default_criteria; {
        inherit tier max_load;
        country = option country;
        features = array features;
      };

      killswitch = with killswitch; {
        inherit enable;
        custom_rules = option (array (map str custom_rules));
      };
    });

    systemd.services.protonvpn-rs = lib.mkIf cfg.enable {
      description = "${serviceName} service";
      before = [ "network.target" ];
      after = [ "network-pre.target" ]
        ++ lib.optionals cfg.requireSops [ "decrypt-sops.service" ];

      path = with pkgs; [ openvpn iptables ];
      serviceConfig = {
        User = "root";
        Group = "root";

        ExecStart =
          "${protonvpn-rs}/bin/protonvpn-rs service start --daemon --verbose";
        Type = "simple";
        RemainAfterExit = true;
      };
      wantedBy = [ "multi-user.target" ];
    };

    systemd.services.networking = lib.mkIf cfg.enable {
      after = [ "${serviceName}.service" ];
      requires = [ "${serviceName}.service" ];
    };
  };
}

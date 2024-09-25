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
        applyFirewallRules = mkEnableOption (mdDoc ''
          Include system firewall rules as custom rules for the vpn killswitch
        '');
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

  imports = [ ./config.nix ];

  config = {
    environment.systemPackages = [ protonvpn-rs ];

    systemd.services.protonvpn-rs = lib.mkIf cfg.enable {
      description = "${serviceName} service";
      after = [ ] ++ lib.optionals cfg.requireSops [ "decrypt-sops.service" ];

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

import SettingsItem from "../settings/settings-item";
import SettingsSection from "../settings/settings-section";
import Input from "../../components/input/input";
import Checkbox from "../../components/checkbox/checkbox";
import Button from "../../components/button/button";
import useSiteMutations from "../../hooks/use-site-mutations";
import { useState } from "react";
import { Hint } from "../../generated-api";
import SettingDescriptions from "@/lib/setting_descriptions";

const NewRemoteSiteSettings = () => {
  const [newName, setNewName] = useState("hostname");
  const [newPort, setNewPort] = useState<number>(80);
  const [https, setHttps] = useState(true);
  const [captureSubdomains, setCaptureSubdomains] = useState(false);
  const [disableTcpTunnelMode, setDisableTcpTunnelMode] = useState(false);
  const [forwardSubdomains, setForwardSubdomains] = useState(false);
  const [H2hints, setH2Hints] = useState<Array<Hint>>([]);

  const { updateRemoteSite } = useSiteMutations();

  const createSite = () => {
    if (!newPort) {
      return;
    }
    updateRemoteSite.mutateAsync({
      siteSettings: {
        host_name: newName,
        backends: [
          {
            address: newName,
            https,
            port: newPort,
            hints: H2hints,
          },
        ],
        capture_subdomains: captureSubdomains,
        disable_tcp_tunnel_mode: disableTcpTunnelMode,
        forward_subdomains: forwardSubdomains,
      },
    });
  };

  return (
    <>
      <SettingsSection noTopSeparator>
        <SettingsItem title="Hostname" subTitle={SettingDescriptions["hostname"]}>
          <Input
            value={newName}
            placeholder="my-site.com"
            onChange={(e) => setNewName(e.target.value)}
          />
        </SettingsItem>
        <SettingsItem title="Port" subTitle={SettingDescriptions["port"]}>
          <Input
            value={newPort}
            onChange={(e) => {
              if (isNaN(Number(e.target.value))) {
                return;
              }
              setNewPort(Number(e.target.value));
            }}
          />
        </SettingsItem>
      </SettingsSection>

      <SettingsSection noTopSeparator>
        <SettingsItem
          labelFor="use_https"
          rowOnly
          title="HTTPS"
          subTitle={SettingDescriptions["https"]}
        >
          <Input
            checked={https}
            onChange={(e) => {
              setHttps(e.target.checked);
            }}
            name="use_https"
            id="use_https"
            type="checkbox"
            style={{ width: "20px", height: "20px" }}
          />
        </SettingsItem>
      </SettingsSection>

      <SettingsSection noTopSeparator>
        <SettingsItem
          labelFor="capture_subdomains"
          rowOnly
          title="Capture sub-domains"
          subTitle={SettingDescriptions["capture_subdomains"]}
        >
          <Input
            onChange={(e) => {
              setCaptureSubdomains(e.target.checked);
            }}
            checked={captureSubdomains}
            type="checkbox"
            name="capture_subdomains"
            id="capture_subdomains"
            style={{ width: "20px", height: "20px" }}
          />
        </SettingsItem>

        <SettingsItem
          rowOnly
          labelFor="disable_tcp_tunnel"
          title="Disable TCP tunnel mode"
          subTitle={SettingDescriptions["disable_tcp_tunnel"]}
        >
          <Input
            type="checkbox"
            checked={disableTcpTunnelMode}
            onChange={(e) => {
              setDisableTcpTunnelMode(e.target.checked);
            }}
            id="disable_tcp_tunnel"
            name="disable_tcp_tunnel"
            style={{ width: "20px", height: "20px" }}
          />
        </SettingsItem>

        <SettingsItem
          rowOnly
          labelFor="forward_subdomains"
          title="Forward sub-domains"
          subTitle={SettingDescriptions["forward_subdomains"]}
        >
          <Input
            type="checkbox"
            checked={forwardSubdomains}
            onChange={(e) => {
              setForwardSubdomains(e.target.checked);
            }}
            id="forward_subdomains"
            name="forward_subdomains"
            style={{ width: "20px", height: "20px" }}
          />
        </SettingsItem>
      </SettingsSection>

      <SettingsItem title="Hints" subTitle={SettingDescriptions["h2_hint"]}/>
      <div
        style={{
          display: "flex",
          gap: "10px",
          flexWrap: "wrap",
          justifyContent: "start",
          marginTop: "10px",
        }}
      >
        <Checkbox
          onClick={() => {
            if (H2hints.includes(Hint.H2)) {
              setH2Hints((old) => [...old.filter((x) => x !== Hint.H2)]);
            } else {
              setH2Hints((old) => [...old, Hint.H2]);
            }
          }}
          checked={H2hints.includes(Hint.H2)}
          title="H2"
        />
        <Checkbox
          onClick={() => {
            if (H2hints.includes(Hint.H2C)) {
              setH2Hints((old) => [...old.filter((x) => x !== Hint.H2C)]);
            } else {
              setH2Hints((old) => [...old, Hint.H2C]);
            }
          }}
          checked={H2hints.includes(Hint.H2C)}
          title="H2C"
        />
        <Checkbox
          onClick={() => {
            if (H2hints.includes(Hint.H2CPK)) {
              setH2Hints((old) => [...old.filter((x) => x !== Hint.H2CPK)]);
            } else {
              setH2Hints((old) => [...old, Hint.H2CPK]);
            }
          }}
          checked={H2hints.includes(Hint.H2CPK)}
          title="H2CPK"
        />
        <Checkbox
          onClick={() => {
            if (H2hints.includes(Hint.NOH2)) {
              setH2Hints((old) => [...old.filter((x) => x !== Hint.NOH2)]);
            } else {
              setH2Hints((old) => [...old, Hint.NOH2]);
            }
          }}
          checked={H2hints.includes(Hint.NOH2)}
          title="NOH2"
        />
      </div>

      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "end",
          marginTop: "20px",
        }}
      >
        <Button
          onClick={createSite}
          style={{
            width: "max-content",
            background: "var(--color7)",
          }}
        >
          Create site
        </Button>
      </div>
    </>
  );
};

export default NewRemoteSiteSettings;

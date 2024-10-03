import SettingsItem from "../settings/settings-item";
import SettingsSection from "../settings/settings-section";
import Input from "../../components/input/input";
import KeyValueInput from "../../components/key-value-input/key-value-input";
import ArgsInput from "../../components/args-input/args-input";
import Button from "../../components/button/button";
import useSiteMutations from "../../hooks/use-site-mutations";
import { useState } from "react";
import { Hint, KvP, LogFormat } from "../../generated-api";
import Checkbox from "../../components/checkbox/checkbox";
import SettingDescriptions from "@/lib/setting_descriptions";

const NewHostedProcessSettings = () => {
  const [newName, setNewName] = useState("hostname");
  const [newPort, setNewPort] = useState<number>(80);
  const [newDir, setNewDir] = useState("");
  const [newBin, setNewBin] = useState("");
  const [https, setHttps] = useState(true);
  const [autoStart, setAutoStart] = useState(false);
  const [captureSubdomains, setCaptureSubdomains] = useState(false);
  const [disableTcpTunnelMode, setDisableTcpTunnelMode] = useState(false);
  const [forwardSubdomains, setForwardSubdomains] = useState(false);
  const [H2hints, setH2Hints] = useState<Array<Hint>>([]);
  const [logFormat, setLogFormat] = useState<LogFormat>(LogFormat.Dotnet);
  const [envVars, setEnvVars] = useState<Array<KvP>>([]);
  const [args, setArgs] = useState<Array<string>>([]);
  const { updateSite } = useSiteMutations();

  const createSite = () => {
    if (!newPort) {
      return;
    }
    updateSite.mutateAsync({
      siteSettings: {
        host_name: newName,
        port: newPort,
        dir: newDir,
        bin: newBin,
        https,
        auto_start: autoStart,
        capture_subdomains: captureSubdomains,
        disable_tcp_tunnel_mode: disableTcpTunnelMode,
        forward_subdomains: forwardSubdomains,
        hints: H2hints,
        log_format: logFormat,
        env_vars: envVars,
        args,
      },
    });
  };

  return (
    <>
      <SettingsSection noTopSeparator>
        <SettingsItem title="Hostname" subTitle={SettingDescriptions["hostname"]}>
          <Input
            placeholder="my-site.com"
            value={newName}
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
      <SettingsSection noTopSeparator noBottomSeparator>
        <SettingsItem
          title="Directory"
          subTitle={SettingDescriptions["directory"]}
        >
          <Input
            placeholder="/var/www/my-site"
            value={newDir}
            onChange={(e) => setNewDir(e.target.value)}
          />
        </SettingsItem>
        <SettingsItem title="Bin" subTitle={SettingDescriptions["binary"]}>
          <Input
            placeholder="my-binary"
            value={newBin}
            onChange={(e) => setNewBin(e.target.value)}
          />
        </SettingsItem>
      </SettingsSection>

      <SettingsSection noTopSeparator noBottomSeparator>
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
          labelFor="auto_start"
          rowOnly
          title="Auto start"
          subTitle={SettingDescriptions["auto_start"]}
        >
          <Input
            id="auto_start"
            checked={autoStart}
            name="auto_start"
            onChange={(e) => {
              setAutoStart(e.target.checked);
            }}
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

      <SettingsSection noTopSeparator>
        <SettingsItem title="Log format" subTitle={SettingDescriptions["log_format"]}>
          <select
            className="text-black rounded pl-3 pr-3"
            value={logFormat}
            onChange={(e) => {
              setLogFormat(e.target.value as LogFormat);
            }}
            name="log_format"
            style={{ height: "32px", width: "100%" }}
          >
            <option value={LogFormat.Standard}>Standard</option>
            <option value={LogFormat.Dotnet}>Dotnet</option>
          </select>
        </SettingsItem>
      </SettingsSection>

      <div style={{ marginTop: "20px" }} />
      <SettingsItem title="Hints" subTitle={SettingDescriptions["h2_hint"]}/>
      <div
        style={{
          display: "flex",
          gap: "10px",
          flexWrap: "wrap",
          justifyContent: "start",
          marginTop: "10px",
          marginBottom: "20px",
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
      <SettingsItem
        title="Environment variables"
        subTitle={SettingDescriptions["env_vars"]}
      />
      <KeyValueInput
        onRemoveKey={(keyName) => {
          setEnvVars(envVars?.filter((key) => key.key !== keyName));
        }}
        onNewKey={(key, originalName) => {
          setEnvVars((old) => [
            ...old.filter((x) => x.key !== originalName),
            key,
          ]);
        }}
        keys={envVars}
      />
      <div style={{ marginBottom: "20px" }} />
      <SettingsItem title="Arguments" subTitle={SettingDescriptions["args"]} />
      <ArgsInput
        onAddArg={(arg, originalValue) => {
          setArgs((old) => [...old.filter((x) => x !== originalValue), arg]);
        }}
        onRemoveArg={(arg: string) => {
          setArgs(args.filter((x) => x !== arg));
        }}
        defaultKeys={args}
      />
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
          style={{ width: "max-content", background: "var(--color7)" }}
        >
          Create site
        </Button>
      </div>
    </>
  );
};

export default NewHostedProcessSettings;

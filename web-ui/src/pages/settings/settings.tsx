import { Suspense, useState } from "react";
import Input from "../../components/input/input";
import SettingsItem from "./settings-item";
import SettingsSection from "./settings-section";
import "react-responsive-modal/styles.css";
import useSettings from "../../hooks/use-settings";
import toast from "react-hot-toast";
import useSettingsMutations from "../../hooks/use-settings-mutations";
import { LogFormat } from "../../generated-api";
import SettingDescriptions from "@/lib/setting_descriptions";
import { EnvVariablesTable } from "@/components/table/env_variables/env_variables";

const SettingsPage = () => {
  return (
    <Suspense fallback={<p>loading settings..</p>}>
      <SettingsPageInner />
    </Suspense>
  );
};

const SettingsPageInner = () => {
  const { updateSettings } = useSettingsMutations();
  const { data: settings } = useSettings();
  const [newIp, setNewIp] = useState(settings.ip);

  const [newRootDir, setNewRootDir] = useState(settings.root_dir);
  const [newPort, setNewPort] = useState(settings.http_port);
  const [newTlsPort, setNewTlsPort] = useState(settings.tls_port);
  const [newPortRangeStart, setNewPortRangeStart] = useState(
    settings.port_range_start
  );

  const updateSetting = (key: string, value: any) => {
    let val =
      Array.isArray(value) || isNaN(value) === false ? value : `${value}`;

    toast.promise(
      updateSettings.mutateAsync({
        ...settings,
        [key]: val,
      }),
      {
        loading: `Updating settings..`,
        success: `Settings updated!`,
        error: `Failed to update settings`,
      }
    );
  };

  return (
    <div style={{ paddingBottom: "50px", maxWidth: "750px" }}>
      <p
        style={{
          textTransform: "uppercase",
          fontSize: ".9rem",
          fontWeight: "bold",
          color: "var(--color2)",
        }}
      >
        Settings
      </p>
      <p style={{ fontSize: ".9rem", marginBottom: "30px" }}>
        General settings that affect all sites
      </p>
      <SettingsSection noTopSeparator noBottomSeparator>
        <SettingsItem
          title="Root directory"
          subTitle={SettingDescriptions["root_dir"]}
        >
          <Input
            withSaveButton
            onSave={(newValue) => {
              updateSetting("root_dir", newValue);
            }}
            type="text"
            originalValue={settings.root_dir}
            value={newRootDir}
            onChange={(e) => setNewRootDir(e.target.value)}
          />
        </SettingsItem>
      </SettingsSection>
      <SettingsSection>
        <SettingsItem
          title="HTTP Port"
          subTitle={SettingDescriptions["default_http_port"]}
          defaultValue="8080"
        >
          <Input
            value={newPort}
            withSaveButton
            originalValue={settings.http_port}
            onSave={(newValue) => {
              updateSetting("http_port", newValue);
            }}
            onChange={(e) => {
              if (isNaN(Number(e.target.value))) {
                return;
              }
              setNewPort(Number(e.target.value));
            }}
          />
        </SettingsItem>
        <SettingsItem
          title="TLS Port"
          subTitle={SettingDescriptions["default_tls_port"]}
          defaultValue="4343"
        >
          <Input
            value={newTlsPort}
            originalValue={settings.tls_port}
            withSaveButton
            onSave={(newValue) => {
              updateSetting("tls_port", newValue);
            }}
            onChange={(e) => {
              if (isNaN(Number(e.target.value))) {
                return;
              }
              setNewTlsPort(Number(e.target.value));
            }}
          />
        </SettingsItem>
        <SettingsItem
          title="IP Address"
          subTitle={SettingDescriptions["proxy_ip"]}
        >
          <Input
            value={newIp}
            originalValue={settings.ip}
            withSaveButton
            onSave={(newValue) => {
              updateSetting("ip", newValue);
            }}
            onChange={(e) => setNewIp(e.target.value)}
          />
        </SettingsItem>
      </SettingsSection>

      <SettingsSection noTopSeparator>
        <SettingsItem
          title="Port range start"
          subTitle={SettingDescriptions["port_range_start"]}
        >
          <Input
            value={newPortRangeStart}
            originalValue={settings.port_range_start}
            withSaveButton
            onSave={(newVal) => updateSetting("port_range_start", newVal)}
            onChange={(e) => {
              if (isNaN(Number(e.target.value))) {
                return;
              }
              setNewPortRangeStart(Number(e.target.value));
            }}
          />
        </SettingsItem>

        <SettingsItem
          title="Use ALPN"
          labelFor="alpn"
          subTitle={SettingDescriptions["use_alpn"]}
          rowOnly
        >
          <Input
            type="checkbox"
            id="alpn"
            checked={settings.alpn}
            onChange={() => updateSetting("alpn", !settings.alpn)}
            style={{ width: "20px", height: "20px" }}
          />
        </SettingsItem>
      </SettingsSection>
      <SettingsSection noTopSeparator>
        <SettingsItem
          title="Autostart"
          rowOnly
          subTitle={SettingDescriptions["default_auto_start"]}
          labelFor="autostart"
        >
          <Input
            id="autostart"
            type="checkbox"
            checked={settings.auto_start}
            onChange={() => updateSetting("auto_start", !settings.auto_start)}
            style={{ width: "20px", height: "20px" }}
          />
        </SettingsItem>
      </SettingsSection>

      <SettingsSection noTopSeparator>
        <SettingsItem
          title="Log level"
          subTitle={SettingDescriptions["log_level"]}
        >
          <select
            className="text-black rounded pl-3 pr-3"
            value={settings.log_level}
            onChange={(e) => {
              updateSetting("log_level", e.target.value);
            }}
            name="loglevel"
            style={{ height: "32px", width: "100%" }}
          >
            <option value="Trace">Trace</option>
            <option value="Debug">Debug</option>
            <option value="Info">Info</option>
            <option value="Warn">Warn</option>
            <option value="Error">Error</option>
          </select>
        </SettingsItem>
        <SettingsItem title="Log format" subTitle={SettingDescriptions["default_log_format"]}>
          <select
            className="text-black rounded pl-3 pr-3"
            value={settings.default_log_format ?? LogFormat.Standard}
            onChange={(e) => {
              updateSetting("default_log_format", e.target.value);
            }}
            name="log_format"
            style={{ height: "32px", width: "100%" }}
          >
            <option value={"Standard"}>Standard</option>
            <option value={"Dotnet"}>Dotnet</option>
          </select>
        </SettingsItem>
      </SettingsSection>

      <SettingsItem vertical
        title="Environment variables"
        subTitle={SettingDescriptions["global_env_vars"]}>
      <EnvVariablesTable keys={settings.env_vars ?? []}         onRemoveKey={(keyName) => {
          updateSetting(
            "env_vars",
            settings.env_vars?.filter((key: any) => key.key !== keyName)
          );
        }}
        onNewKey={(key, originalName) => {
          updateSetting("env_vars", [
            ...settings.env_vars.filter(
              (x: any) => x.key !== key.key && x.key !== originalName
            ),
            { key: key.key, value: key.value },
          ]);
        }}/>
</SettingsItem>

    </div>
  );
};

export default SettingsPage;

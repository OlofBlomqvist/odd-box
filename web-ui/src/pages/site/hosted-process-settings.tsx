import SettingsItem from "../settings/settings-item";
import SettingsSection from "../settings/settings-section";
import Input from "../../components/input/input";
import "./style.css";
import Button from "../../components/button/button";
import useSiteMutations from "../../hooks/use-site-mutations";
import toast from "react-hot-toast";
import { useState } from "react";
import { useRouter } from "@tanstack/react-router";
import { Hint, InProcessSiteConfig, LogFormat } from "../../generated-api";
import OddModal from "../../components/modal/modal";
import Checkbox from "@/components/checkbox/checkbox";
import SettingDescriptions from "@/lib/setting_descriptions";
import { EnvVariablesTable } from "@/components/table/env_variables/env_variables";
import { ArgumentsTable } from "@/components/table/arguments/arguments";

const HostedProcessSettings = ({ site }: { site: InProcessSiteConfig }) => {
  const { updateSite, deleteSite } = useSiteMutations();
  const [newName, setNewName] = useState(site.host_name);
  const [newPort, setNewPort] = useState(site.port ?? 8080);
  const [newDir, setNewDir] = useState(site.dir ?? undefined);
  const [newBin, setNewBin] = useState(site.bin);
  const [showConfirmDeleteModal, setShowConfirmDeleteModal] = useState(false);
  const router = useRouter();

  const updateSetting = (key: string, value: any) => {
    let val =
      Array.isArray(value) || isNaN(value) === false ? value : `${value}`;

    toast.promise(
      updateSite.mutateAsync({
        hostname: site.host_name,
        siteSettings: {
          ...site,
          [key]: val,
        },
      }),
      {
        loading: `Updating settings.. [${site.host_name}]`,
        success: `Settings updated! [${site.host_name}]`,
        error: (e) => `${e}`,
      }
    );
  };

  return (
    <div
      key={site.host_name}
      style={{ paddingBottom: "50px", maxWidth: "750px" }}
      onSubmit={(e) => {
        e.preventDefault();
      }}
    >
      <SettingsSection noTopSeparator>
        <SettingsItem
          title="Hostname"
          subTitle={SettingDescriptions["hostname_frontend"]}
        >
          <Input
            originalValue={site.host_name}
            onSave={(newValue) => {
              updateSetting("host_name", newValue);
            }}
            withSaveButton
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
          />
        </SettingsItem>
        <SettingsItem title="Port" subTitle={SettingDescriptions["port"]}>
          <Input
            originalValue={site.port ?? 8080}
            withSaveButton
            onSave={(newValue) => {
              updateSetting("port", newValue);
            }}
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
          title="Directory"
          subTitle={SettingDescriptions["directory"]}
        >
          <Input
            value={newDir}
            withSaveButton
            originalValue={site.dir ?? undefined}
            onSave={(newValue) => {
              updateSetting("dir", newValue);
            }}
            onChange={(e) => setNewDir(e.target.value)}
          />
        </SettingsItem>
        <SettingsItem
          title="Bin"
          subTitle={SettingDescriptions["binary"]}
        >
          <Input
            value={newBin}
            withSaveButton
            originalValue={site.bin}
            onSave={(newValue) => {
              updateSetting("bin", newValue);
            }}
            onChange={(e) => setNewBin(e.target.value)}
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
            checked={Boolean(site.https)}
            onChange={() => {
              updateSetting("https", !site.https);
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
            checked={Boolean(site.auto_start)}
            name="auto_start"
            onChange={() => {
              updateSetting("auto_start", !site.auto_start);
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
            onChange={() => {
              updateSetting("capture_subdomains", !site.capture_subdomains);
            }}
            checked={Boolean(site.capture_subdomains)}
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
            checked={Boolean(site.disable_tcp_tunnel_mode)}
            onChange={() => {
              updateSetting(
                "disable_tcp_tunnel_mode",
                !site.disable_tcp_tunnel_mode
              );
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
            checked={Boolean(site.forward_subdomains)}
            onChange={() => {
              updateSetting("forward_subdomains", !site.forward_subdomains);
            }}
            id="forward_subdomains"
            name="forward_subdomains"
            style={{ width: "20px", height: "20px" }}
          />
        </SettingsItem>
      </SettingsSection>

      <SettingsSection noTopSeparator noBottomSeparator>
        <SettingsItem
          title="H2 Hints"
          subTitle={SettingDescriptions["h2_hint"]}
        ></SettingsItem>
<div
          style={{
            display: "flex",
            gap: "10px",
            flexWrap: "wrap",
            justifyContent: "start",
          }}
        >
          <Checkbox
            onClick={() => {
              updateSetting("hints", site.hints?.includes(Hint.H2) ? site.hints.filter((x) => x !== Hint.H2) : [...(site.hints ?? []), Hint.H2]);
            }}
            checked={Boolean(site?.hints?.includes(Hint.H2))}
            title="H2"
          />
          <Checkbox
            onClick={() => {
              updateSetting("hints", site.hints?.includes(Hint.H2C) ? site.hints.filter((x) => x !== Hint.H2C) : [...(site.hints ?? []), Hint.H2C]);
            }}
            checked={Boolean(site?.hints?.includes(Hint.H2C))}
            title="H2C"
          />
          <Checkbox
            onClick={() => {
              updateSetting("hints", site.hints?.includes(Hint.H2CPK) ? site.hints.filter((x) => x !== Hint.H2CPK) : [...(site.hints ?? []), Hint.H2CPK]);
            }}
            checked={Boolean(site?.hints?.includes(Hint.H2CPK))}
            title="H2CPK"
          />
          <Checkbox
            onClick={() => {
              updateSetting("hints", site.hints?.includes(Hint.NOH2) ? site.hints.filter((x) => x !== Hint.NOH2) : [...(site.hints ?? []), Hint.NOH2]);
            }}
            checked={Boolean(site?.hints?.includes(Hint.NOH2))}
            title="NOH2"
          />
        </div>
        <SettingsItem title="Log format" subTitle={SettingDescriptions["log_format"]}>
          <select
            className="text-black rounded pl-3 pr-3"
            value={site.log_format ?? LogFormat.Standard}
            onChange={(e) => {
              updateSetting("log_format", e.target.value);
            }}
            name="log_format"
            style={{ height: "30px", width: "100%" }}
          >
            <option value={LogFormat.Standard}>Standard</option>
            <option value={LogFormat.Dotnet}>Dotnet</option>
          </select>
        </SettingsItem>
      </SettingsSection>

      <div style={{ marginBottom: "20px" }}>
        <SettingsItem
          labelFor="exclude_from_start_all"
          rowOnly
          title="Exclude from 'start all'"
          subTitle={SettingDescriptions["exclude_from_start_all"]}
        >
          <Input
            checked={Boolean(site.exclude_from_start_all)}
            onChange={() => {
              updateSetting(
                "exclude_from_start_all",
                !site.exclude_from_start_all
              );
            }}
            id="exclude_from_start_all"
            type="checkbox"
            style={{ width: "20px", height: "20px" }}
          />
        </SettingsItem>
      </div>




<SettingsSection noBottomSeparator>
      <SettingsItem vertical
        title="Environment variables"
        subTitle={SettingDescriptions["env_vars"]}>
      <EnvVariablesTable keys={site.env_vars ?? []}                 onRemoveKey={(keyName) => {
          updateSetting(
            "env_vars",
            site.env_vars?.filter((key) => key.key !== keyName)
          );
        }}
        onNewKey={(key, originalName) => {
          updateSetting("env_vars", [
            ...(site.env_vars?.filter(
              (x) => x.key !== key.key && x.key !== originalName
            ) ?? []),
            { key: key.key, value: key.value },
          ]);
        }}/>
</SettingsItem>
</SettingsSection>

<SettingsSection noBottomSeparator noTopSeparator>
      <SettingsItem vertical
        title="Arguments"
        subTitle={SettingDescriptions["args"]}>
      <ArgumentsTable onAddArg={(arg, originalValue) => {
          updateSetting("args", [
            ...(site.args?.filter((x) => x !== originalValue) ?? []),
            arg,
          ]);
        }}
        onRemoveArg={(arg: string) => {
          updateSetting("args", [
            ...(site.args?.filter((x) => x !== arg) ?? []),
          ]);
        }}
        defaultKeys={site.args ?? []}/>
</SettingsItem>
</SettingsSection>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "end",
          marginTop: "20px",
        }}
      >
        <Button
          onClick={() => {
            setShowConfirmDeleteModal(true);
          }}
          style={{ width: "max-content" }}
          dangerButton
        >
          Delete site
        </Button>
      </div>

      <OddModal
        show={showConfirmDeleteModal}
        onClose={() => setShowConfirmDeleteModal(false)}
        title="Delete"
        subtitle={`Are you sure you want to delete the site '${site.host_name}'?`}
      >
        <div style={{ display: "flex", gap: "20px", marginTop: "10px" }}>
          <Button secondary onClick={() => setShowConfirmDeleteModal(false)}>
            Cancel
          </Button>
          <Button
            onClick={() => {
              toast.promise(
                deleteSite.mutateAsync(
                  { hostname: site.host_name },
                  {
                    onSuccess: () => {
                      setShowConfirmDeleteModal(false);
                      router.navigate({ to: "/" });
                    },
                  }
                ),
                {
                  loading: `Deleting site.. [${site.host_name}]`,
                  success: () => {
                    router.navigate({ to: "/" });
                    return `Site deleted! [${site.host_name}]`;
                  },
                  error: (e) => `${e}`,
                }
              );
            }}
            dangerButton
          >
            Yes, delete
          </Button>
        </div>
      </OddModal>
    </div>
  );
};

export default HostedProcessSettings;

import SettingsItem from "../settings/settings-item";
import SettingsSection from "../settings/settings-section";
import Input from "../../components/input/input";
import Button from "../../components/button/button";
import useSiteMutations from "../../hooks/use-site-mutations";
import toast from "react-hot-toast";
import { useState } from "react";
import { useRouter } from "@tanstack/react-router";
import { Hint, InProcessSiteConfig, LogFormat } from "../../generated-api";
import Checkbox from "@/components/checkbox/checkbox";
import SettingDescriptions from "@/lib/setting_descriptions";
import { ConfirmationDialog } from "@/components/dialog/confirm/confirm";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import useSettings from "@/hooks/use-settings";
import {
  envVarsStringToArray,
  envVarsToString,
} from "@/lib/env_vars_to_string";

const HostedProcessSettings = ({ site }: { site: InProcessSiteConfig }) => {
  const { updateSite, deleteSite } = useSiteMutations();
  const { data: settings } = useSettings();
  const [newName, setNewName] = useState(site.host_name);
  const [newArgs, setNewArgs] = useState((site.args ?? []).join(";"));
  const [newEnvVars, setNewEnvVars] = useState(
    envVarsToString(site.env_vars ?? [])
  );
  const [newPort, setNewPort] = useState<string>(`${site.port ?? ""}`);
  const [newDir, setNewDir] = useState(site.dir ?? undefined);
  const [newBin, setNewBin] = useState(site.bin);
  const [showConfirmDeleteModal, setShowConfirmDeleteModal] = useState(false);
  const router = useRouter();

  const updateSetting = (key: string, value: any) => {
    let val =
      value === undefined || Array.isArray(value) || isNaN(value) === false ? value : `${value}`;

    toast.promise(
      updateSite.mutateAsync({
        hostname: site.host_name,
        siteSettings: {
          ...site,
          [key]: key === "port" ? (val === "" ? undefined : Number(val)) : val,
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
    <main
      className="grid flex-1 items-start gap-4 sm:py-0 md:gap-8 max-w-[900px]"
      key={site.host_name}
      style={{}}
      onSubmit={(e) => {
        e.preventDefault();
      }}
    >
      <Card>
        <CardHeader>
          <CardTitle>Settings</CardTitle>
          <CardDescription>
            Current configuration for{" "}
            <span className="font-bold text-[var(--accent-text)]">
              {site.host_name}
            </span>
          </CardDescription>
        </CardHeader>
        <CardContent>
          <SettingsSection marginTop="0px" noTopSeparator>
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
            <SettingsItem
              title="Port"
              defaultValue={settings.http_port}
              subTitle={SettingDescriptions["port"]}
            >
              <Input
                originalValue={`${site.port ?? ""}`}
                withSaveButton
                placeholder={settings.http_port.toString()}
                onSave={(newValue) => {
                  updateSetting("port", newValue);
                }}
                value={newPort}
                onChange={(e) => {
                  if (e.target.value.length && isNaN(Number(e.target.value))) {
                    return;
                  }
                  setNewPort(e.target.value);
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
            <SettingsItem title="Bin" subTitle={SettingDescriptions["binary"]}>
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
              labelFor="terminate_tls"
              title="Always terminate TLS"
              subTitle={SettingDescriptions["terminate_tls"]}
            >
              <Input
                type="checkbox"
                checked={Boolean(site.terminate_tls)}
                onChange={() => {
                  updateSetting(
                    "terminate_tls",
                    !site.terminate_tls
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
              title="Hints"
              subTitle={SettingDescriptions["site_hints"]}
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
                  updateSetting(
                    "hints",
                    site.hints?.includes(Hint.H1)
                      ? site.hints.filter((x) => x !== Hint.H1)
                      : [...(site.hints ?? []), Hint.H1]
                  );
                }}
                checked={Boolean(site?.hints?.includes(Hint.H1))}
                title="H1"
              />
              <Checkbox
                onClick={() => {
                  updateSetting(
                    "hints",
                    site.hints?.includes(Hint.H2)
                      ? site.hints.filter((x) => x !== Hint.H2)
                      : [...(site.hints ?? []), Hint.H2]
                  );
                }}
                checked={Boolean(site?.hints?.includes(Hint.H2))}
                title="H2"
              />
              <Checkbox
                onClick={() => {
                  updateSetting(
                    "hints",
                    site.hints?.includes(Hint.H2C)
                      ? site.hints.filter((x) => x !== Hint.H2C)
                      : [...(site.hints ?? []), Hint.H2C]
                  );
                }}
                checked={Boolean(site?.hints?.includes(Hint.H2C))}
                title="H2C"
              />
              <Checkbox
                onClick={() => {
                  updateSetting(
                    "hints",
                    site.hints?.includes(Hint.H2CPK)
                      ? site.hints.filter((x) => x !== Hint.H2CPK)
                      : [...(site.hints ?? []), Hint.H2CPK]
                  );
                }}
                checked={Boolean(site?.hints?.includes(Hint.H2CPK))}
                title="H2CPK"
              />
            </div>
            <SettingsItem
              title="Log format"
              subTitle={SettingDescriptions["log_format"]}
            >
              <select
                className="text-black rounded pl-3 pr-3 bg-white border border-[var(--border)]"
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
            <SettingsItem
              vertical
              title="Environment variables"
              subTitle={
                "Semicolon separated list of environment variables to be set on process start."
              }
              dangerText={
                "Example: my_variable=my_value;my_other_variable=my_other_value"
              }
            >
              <Input
                withSaveButton
                disableSaveButton={updateSite.isPending}
                originalValue={envVarsToString(site.env_vars ?? [])}
                value={newEnvVars}
                onSave={() => {
                  updateSetting("env_vars", envVarsStringToArray(newEnvVars));
                }}
                onChange={(e) => {
                  setNewEnvVars(e.target.value);
                }}
              />
            </SettingsItem>
          </SettingsSection>

          <SettingsSection noBottomSeparator noTopSeparator>
            <SettingsItem
              vertical
              title="Arguments"
              subTitle={
                "Semicolon separated list of arguments. Applied in the same order shown here."
              }
            >
              <Input
                withSaveButton
                disableSaveButton={updateSite.isPending}
                originalValue={(site.args ?? []).join(";")}
                value={newArgs}
                onSave={() => {
                  updateSetting("args", newArgs === "" ? undefined : newArgs.split(";"));
                }}
                onChange={(e) => {
                  setNewArgs(e.target.value);
                }}
              />
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

          <ConfirmationDialog
            isDangerAction
            isSuccessLoading={deleteSite.isPending}
            onClose={() => setShowConfirmDeleteModal(false)}
            inProgressText="Deleting.."
            onConfirm={async () => {
              await deleteSite.mutateAsync(
                { hostname: site.host_name },
                {
                  onSuccess: () => {
                    setShowConfirmDeleteModal(false);
                    router.navigate({ to: "/", search: { type: "processes" } });
                  },
                }
              );
            }}
            show={showConfirmDeleteModal}
            title="Delete"
            yesBtnText="Yes, delete it"
            subtitle={
              <span>
                Are you sure you want to delete{" "}
                <span className="font-bold text-[var(--accent-text)]">
                  {site.host_name}
                </span>
                ?
              </span>
            }
          />
        </CardContent>
      </Card>
    </main>
  );
};

export default HostedProcessSettings;

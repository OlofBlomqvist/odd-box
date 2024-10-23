import SettingsItem from "../settings/settings-item";
import SettingsSection from "../settings/settings-section";
import Input from "../../components/input/input";
import Button from "../../components/button/button";
import { useState } from "react";
import { Link, useRouter } from "@tanstack/react-router";
import { DirServer } from "../../generated-api";
import SettingDescriptions from "@/lib/setting_descriptions";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import useSettings from "@/hooks/use-settings";
import { ConfirmationDialog } from "@/components/dialog/confirm/confirm";
import useSiteMutations from "@/hooks/use-site-mutations";
import toast from "react-hot-toast";
import { getUrlFriendlyUrl } from "@/lib/get_url_friendly_url";

const DirServerSettings = ({ site }: { site: DirServer }) => {
  const { deleteSite, updateDirServer } = useSiteMutations();

  const [newName, setNewName] = useState(site?.host_name);
  const { data: settings } = useSettings();
  const [newDir, setNewDir] = useState(site.dir);

  const [showConfirmDeleteModal, setShowConfirmDeleteModal] = useState(false);

  const router = useRouter();

  const updateSetting = (key: string, value: any) => {
    let val =
      Array.isArray(value) || isNaN(value) === false ? value : `${value}`;

    toast.promise(
      updateDirServer.mutateAsync(
        {
          hostname: site.host_name,
          siteSettings: {
            ...site,
            [key]: val,
          },
        },
        {
          onSettled(_data, _error, vars) {
            if (vars.hostname !== vars.siteSettings.host_name) {
              router.navigate({
                to: `/site`,
                search: {
                  tab: 1,
                  hostname: getUrlFriendlyUrl(vars.siteSettings.host_name),
                },
              });
            }
          },
        }
      ),
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
      <Card className="mb-8">
        <CardHeader>
          <CardTitle>Directory server settings</CardTitle>
          <CardDescription>
            General configuration for{" "}
            <span className="font-bold text-[var(--accent-text)]">
              {site.host_name}
            </span>
          </CardDescription>
        </CardHeader>
        <CardContent>
          <>
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
                  placeholder="my-server.local"
                  onChange={(e) => setNewName(e.target.value)}
                />
              </SettingsItem>
              <SettingsItem
                title="Directory"
                subTitle={SettingDescriptions["directory"]}
              >
                <Input
                  originalValue={site.dir}
                  onSave={(newValue) => {
                    updateSetting("dir", newValue);
                  }}
                  withSaveButton
                  value={newDir}
                  placeholder="/home/me/mysite"
                  onChange={(e) => setNewDir(e.target.value)}
                />
              </SettingsItem>
              <SettingsItem
                dangerText={
                  <p className="text-[.8rem]">
                    This is the HTTP port configured for all sites.
                    <br />
                    You can change it on the{" "}
                    <Link
                      className="text-[var(--accent-text)] underline cursor-pointer"
                      to={"/settings"}
                    >
                      general settings
                    </Link>{" "}
                    page.
                  </p>
                }
                title="HTTP Port"
              >
                <Input value={settings.http_port} readOnly disabled />
              </SettingsItem>
              <SettingsItem
                dangerText={
                  <p className="text-[.8rem]">
                    This is the TLS port configured for all sites.
                    <br />
                    You can change it on the{" "}
                    <Link
                      className="text-[var(--accent-text)] underline cursor-pointer"
                      to={"/settings"}
                    >
                      general settings
                    </Link>{" "}
                    page.
                  </p>
                }
                title="TLS Port"
              >
                <Input value={settings.tls_port} readOnly disabled />
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
                    updateSetting(
                      "capture_subdomains",
                      !site.capture_subdomains
                    );
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
                labelFor="enable_directory_browsing"
                title="Enable directory browsing"
                subTitle={SettingDescriptions["enable_directory_browsing"]}
              >
                <Input
                  type="checkbox"
                  onChange={() => {
                    updateSetting(
                      "enable_directory_browsing",
                      !site.enable_directory_browsing
                    );
                  }}
                  checked={Boolean(site.enable_directory_browsing)}
                  id="enable_directory_browsing"
                  name="enable_directory_browsing"
                  style={{ width: "20px", height: "20px" }}
                />
              </SettingsItem>

              <SettingsItem
                rowOnly
                labelFor="lets_encrypt"
                title="Enable Lets-Encrypt"
                dangerText={
                  <p className="text-[.8rem]">
                    Note: You need to have a valid email address configured
                    under{" "}
                    <Link
                      className="text-[var(--accent-text)] underline cursor-pointer"
                      to={"/settings"}
                    >
                      general settings
                    </Link>{" "}
                    to use this.
                  </p>
                }
              >
                <Input
                  disabled={!settings.lets_encrypt_account_email && !site.enable_lets_encrypt}
                  type="checkbox"
                  title={
                    !settings.lets_encrypt_account_email
                      ? "You need to add a valid email address under general settings to enable this."
                      : undefined
                  }
                  onChange={() => {
                    updateSetting(
                      "enable_lets_encrypt",
                      !site.enable_lets_encrypt
                    );
                  }}
                  checked={Boolean(site.enable_lets_encrypt)}
                  id="lets_encrypt"
                  name="lets_encrypt"
                  style={{ width: "20px", height: "20px" }}
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
              onClose={() => setShowConfirmDeleteModal(false)}
              onConfirm={() => {
                setShowConfirmDeleteModal(false);
                deleteSite.mutateAsync(
                  { hostname: site.host_name },
                  {
                    onSuccess: () => {
                      setShowConfirmDeleteModal(false);
                      router.navigate({
                        to: "/",
                        search: { type: "processes" },
                      });
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
          </>
        </CardContent>
      </Card>
    </main>
  );
};

export default DirServerSettings;

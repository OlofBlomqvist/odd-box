import SettingsItem from "../settings/settings-item";
import SettingsSection from "../settings/settings-section";
import Input from "../../components/input/input";
import Button from "../../components/button/button";
import useSiteMutations from "../../hooks/use-site-mutations";
import { useState } from "react";
import SettingDescriptions from "@/lib/setting_descriptions";
import useSettings from "@/hooks/use-settings";
import { Link, useRouter } from "@tanstack/react-router";
import { getUrlFriendlyUrl } from "@/lib/get_url_friendly_url";

const NewDirServerSettings = () => {
  const [newName, setNewName] = useState("");
  const [newDir, setNewDir] = useState("");
  const [captureSubdomains, setCaptureSubdomains] = useState(false);
  const [enableDirectoryBrowsing, setEnableDirectoryBrowsing] = useState(false);
  const [enableLetsEncrypt, setEnableLetsEncrypt] = useState(false);
  const {data:settings} = useSettings();

  const router = useRouter();
  const { updateDirServer } = useSiteMutations();

  const createSite = () => {
    
    updateDirServer.mutateAsync({
      siteSettings: {
        host_name: newName,
        dir: newDir,
        capture_subdomains: captureSubdomains,
        enable_directory_browsing: enableDirectoryBrowsing,
        enable_lets_encrypt: enableLetsEncrypt,
      },
    }, {
      onSettled(_data, _error, vars) {
        if (vars.hostname !== vars.siteSettings.host_name) {
          router.navigate({
            to: `/site`,
            search: { tab: 1, hostname: getUrlFriendlyUrl(vars.siteSettings.host_name) },
          });
        }
      },
    });  

   
  };

  return (
    <>
      <SettingsSection marginTop="0px" noTopSeparator>

        <SettingsItem
          title="Hostname"
          subTitle={SettingDescriptions["hostname_frontend"]}
        >
          <Input
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
            value={newDir}
            placeholder="/home/me/mysite"
            onChange={(e) => setNewDir(e.target.value)}
          />
        </SettingsItem>
        <SettingsItem dangerText={
                    <span className="text-[.8rem]">
                    This is the HTTP port configured for all sites, you can change it on the <Link className="text-[var(--accent-text)] underline cursor-pointer" to={"/settings"}>general settings</Link> page.
                  </span>
        } title="HTTP Port">
          <Input
            value={settings.http_port}
            readOnly
            disabled
          />
        </SettingsItem>
        <SettingsItem dangerText={
                    <span className="text-[.8rem]">
                    This is the TLS port configured for all sites, you can change it on the <Link className="text-[var(--accent-text)] underline cursor-pointer" to={"/settings"}>general settings</Link> page.
                  </span>
        } title="TLS Port">
          <Input
            value={settings.tls_port}
            readOnly
            disabled
            
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
          labelFor="enable_directory_browsing"
          title="Enable directory browsing"
          subTitle={SettingDescriptions["enable_directory_browsing"]}
        >
          <Input
            type="checkbox"
            checked={enableDirectoryBrowsing}
            onChange={(e) => {
              setEnableDirectoryBrowsing(e.target.checked);
            }}
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
          disabled={!settings.lets_encrypt_account_email}
            type="checkbox"
            checked={enableLetsEncrypt}
            onChange={(e) => {
              setEnableLetsEncrypt(e.target.checked);
            }}
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

export default NewDirServerSettings;

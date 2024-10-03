import { useState } from "react";
import SettingsItem from "../settings/settings-item";
import SettingsSection from "../settings/settings-section";
import NewHostedProcessSettings from "./new-hosted-process-settings";
import NewRemoteSiteSettings from "./new-remote-site-settings";
import SettingDescriptions from "@/lib/setting_descriptions";

const NewSitePage = () => {
  const [siteType, setSiteType] = useState("HostedProcess");
  return (
    <>
      <p
        style={{
          textTransform: "uppercase",
          fontSize: ".9rem",
          fontWeight: "bold",
          color: "var(--color2)",
        }}
      >
        new site
      </p>
      <div
        style={{ paddingBottom: "50px", maxWidth: "750px" }}
        onSubmit={(e) => {
          e.preventDefault();
        }}
      >
        <SettingsSection>
          <SettingsItem
            title="Type of site"
            subTitle={SettingDescriptions["site_type"]}
          >
            <select
              className="text-black rounded pl-3 pr-3"
              value={siteType}
              onChange={(e) => {
                setSiteType(e.target.value);
              }}
              style={{ height: "30px", width: "100%" }}
            >
              <option value="HostedProcess">Hosted process</option>
              <option value="RemoteSite">Remote site</option>
            </select>
          </SettingsItem>
        </SettingsSection>
        {siteType === "HostedProcess" && <NewHostedProcessSettings />}
        {siteType === "RemoteSite" && <NewRemoteSiteSettings />}
      </div>
    </>
  );
};

export default NewSitePage;

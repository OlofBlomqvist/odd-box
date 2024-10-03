import "./style.css";
import useLiveLog, { TLogMessage } from "../../hooks/use-live-log";
import { useState } from "react";
import SettingsSection from "../settings/settings-section";
import SettingsItem from "../settings/settings-item";
import useHostedSites from "../../hooks/use-hosted-sites";
import { useRemoteSites } from "../../hooks/use-remote-sites";
import { InProcessSiteConfig, RemoteSiteConfig } from "../../generated-api";
import Checkbox from "@/components/checkbox/checkbox";

const SiteLogs = ({
  hostedProcess,
  remoteSite,
}: {
  hostedProcess?: InProcessSiteConfig;
  remoteSite?: RemoteSiteConfig;
}) => {
  const { data: hostedSites } = useHostedSites();
  const { data: remoteSites } = useRemoteSites();

  const [selectedSite, setSelectedSite] = useState(
    hostedProcess?.host_name ?? remoteSite?.host_name ?? "all"
  );
  const { messageHistory } = useLiveLog();
  const [lvlFilter, setLvlFilter] = useState<Array<string>>([
    "info",
    "warn",
    "error",
  ]);

  let filteredMessages: TLogMessage[] = [];

  filteredMessages = messageHistory.filter(
    (x) =>
      x.msg !== "" &&
      (selectedSite === "all" || x.thread === selectedSite) &&
      lvlFilter.includes(x.lvl.toLowerCase())
  );

  return (
    <div style={{ paddingBottom: "40px", maxWidth: "1000px" }}>
      <SettingsSection noTopSeparator noBottomSeparator>
        <SettingsItem
          title="Site"
          subTitle="Which site do you want to see messages from"
        >
          <select
            className="text-black rounded pl-3 pr-3"
            onChange={(e) => setSelectedSite(e.target.value)}
            style={{ height: "30px", width: "100%", minWidth: "200px" }}
            defaultValue={
              hostedProcess?.host_name ?? remoteSite?.host_name ?? "all"
            }
          >
            <option value="all">All sites</option>
            <option value="system">System messages</option>
            {hostedSites.map((x) => (
              <option key={x.host_name}>{x.host_name}</option>
            ))}
            {remoteSites.map((x) => (
              <option key={x.host_name}>{x.host_name}</option>
            ))}
          </select>
        </SettingsItem>
        <SettingsItem
          title="Filter messages"
          subTitle="Which type of messages do you want to see"
        >
          <div style={{ display: "flex", gap: "10px", marginBottom: "10px" }}>
            <Checkbox
              title="Info"
              onClick={() => {
                if (lvlFilter.includes("info")) {
                  setLvlFilter((old) => [...old.filter((x) => x !== "info")]);
                } else {
                  setLvlFilter((old) => [...old, "info"]);
                }
              }}
              checked={lvlFilter.includes("info")}
            ></Checkbox>
            <Checkbox
              title="Warning"
              onClick={() => {
                if (lvlFilter.includes("warn")) {
                  setLvlFilter((old) => [...old.filter((x) => x !== "warn")]);
                } else {
                  setLvlFilter((old) => [...old, "warn"]);
                }
              }}
              checked={lvlFilter.includes("warn")}
            ></Checkbox>
            <Checkbox
              title="Error"
              onClick={() => {
                if (lvlFilter.includes("error")) {
                  setLvlFilter((old) => [...old.filter((x) => x !== "error")]);
                } else {
                  setLvlFilter((old) => [...old, "error"]);
                }
              }}
              checked={lvlFilter.includes("error")}
            ></Checkbox>
          </div>
        </SettingsItem>
      </SettingsSection>

      <div style={{ display: "flex", gap: "10px", padding: "0px 10px" }}>
        <p
          style={{
            fontSize: ".9rem",
            color: "var(--color3)",
            height: "40px",
            alignContent: "center",
            justifySelf: "center",
            width: "70px",
            minWidth: "70px",
          }}
          className="hide-when-small"
        >
          LEVEL
        </p>
        <p
          style={{
            fontSize: ".9rem",
            color: "var(--color3)",
            height: "40px",
            alignContent: "center",
            width: "70px",
            minWidth: "70px",
          }}
        >
          TIME
        </p>
        <p
          style={{
            fontSize: ".9rem",
            color: "var(--color3)",
            height: "40px",
            alignContent: "center",
          }}
        >
          MESSAGE
        </p>
      </div>
      <div
        style={{
          background: "#00000033",
          border: "1px solid #ffffff44",
          borderRadius: "5px",
          minHeight: "50px",
        }}
      >
        {filteredMessages.map((x, i) => (
          <div className="log-row" key={`${x.timestamp}_${x.msg}`}>
            <div
              className="hide-when-small"
              style={{
                gridRow: 2 + i,
                fontSize: ".9rem",
                justifyContent: "stretch",
                alignContent: "start",
                alignSelf: "start",
                height: "100%",
                width: "70px",
                minWidth: "70px",
              }}
            >
              <p
                style={{
                  background:
                    x.lvl === "ERROR"
                      ? "var(--color1)"
                      : x.lvl === "WARN"
                        ? "var(--color6)"
                        : "#889fae",
                  userSelect: "none",
                  color: "white",
                  padding: "4px 8px",
                  borderRadius: "8px",
                  textTransform: "uppercase",
                  textAlign: "center",
                }}
              >
                {x.lvl}
              </p>
            </div>

            <div
              style={{
                position: "relative",
                gridRow: 2 + i,
                fontSize: ".8rem",
                display: "grid",
                justifyContent: "stretch",
                alignContent: "start",
                alignSelf: "start",
                height: "100%",
                width: "70px",
                minWidth: "70px",
              }}
            >
              <p
                style={{
                  padding: "4px 0px",
                  fontSize: ".9rem",
                  alignSelf: "start",
                  alignContent: "center",
                }}
              >
                {x.timestamp}
              </p>
            </div>

            <div
              style={{
                position: "relative",
                gridRow: 2 + i,
                fontSize: ".8rem",
                display: "grid",
                justifyContent: "stretch",
                alignContent: "start",
                width: "100%",
                alignSelf: "start",
                height: "100%",
              }}
            >
              <p
                style={{
                  padding: "4px 0px",
                  fontSize: ".9rem",
                  alignSelf: "start",
                  overflow: "auto",
                }}
              >
                {selectedSite === "all" && (
                  <span
                    style={{ color: "var(--color2)", fontWeight: "bold" }}
                  >{`[${x.thread}] `}</span>
                )}
                {x.msg}
              </p>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export default SiteLogs;

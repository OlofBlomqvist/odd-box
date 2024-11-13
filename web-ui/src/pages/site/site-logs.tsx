import { useState } from "react";
import SettingsSection from "../settings/settings-section";
import SettingsItem from "../settings/settings-item";
import useHostedSites from "../../hooks/use-hosted-sites";
import { useRemoteSites } from "../../hooks/use-remote-sites";
import Checkbox from "@/components/checkbox/checkbox";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Route } from "@/routes/logs";
import { useRouter } from "@tanstack/react-router";
import { useLiveLogsContext } from "@/providers/live_logs";

const SiteLogs = ({
  host
}:{
  host?:string
}) => {
  const router = useRouter();
  const hostname = host ?? Route.useSearch()?.hostname ?? "all";
  const { data: hostedSites } = useHostedSites();
  const { data: remoteSites } = useRemoteSites();

  let { messageHistory } = useLiveLogsContext();
  const [lvlFilter, setLvlFilter] = useState<Array<string>>([
    "info",
    "warn",
    "error",
    "debug",
    "trace"
  ]);

  const filteredMessages = messageHistory.filter(
    (x) =>
      x.msg !== "" &&
      (hostname === "all" || x.thread === hostname) &&
      lvlFilter.includes(x.lvl.toLowerCase())
  );

  return (
    <Card className="mb-8">
      <CardHeader>
        <CardTitle>Logs</CardTitle>
        <CardDescription>
          Monitoring logs for{" "}
          <span className="font-bold text-[var(--accent-text)]">
            {hostname === "all"
              ? "all sites"
              : hostname === "system"
                ? "system messages"
                : hostname}
          </span>
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div>
          <SettingsSection marginTop={"0px"} noTopSeparator noBottomSeparator>
            <SettingsItem
              title="Site"
              subTitle="Which site do you want to see messages from"
            >
              <select
                className="text-black rounded pl-3 pr-3 bg-white border border-[var(--border)]" disabled={Boolean(host)}
                onChange={(e) => {
                  router.navigate({
                    search: {
                      hostname: e.target.value,
                    },
                    replace: true
                  });
                }}
                style={{ height: "30px", width: "100%", minWidth: "200px" }}
                defaultValue={hostname ?? "all"}
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
              <div
                style={{ display: "flex", gap: "10px", marginBottom: "10px" }}
              >
                 <Checkbox
                  title="Trace"
                  onClick={() => {
                    if (lvlFilter.includes("trace")) {
                      setLvlFilter((old) => [
                        ...old.filter((x) => x !== "trace"),
                      ]);
                    } else {
                      setLvlFilter((old) => [...old, "trace"]);
                    }
                  }}
                  checked={lvlFilter.includes("trace")}
                ></Checkbox>
                 <Checkbox
                  title="Debug"
                  onClick={() => {
                    if (lvlFilter.includes("debug")) {
                      setLvlFilter((old) => [
                        ...old.filter((x) => x !== "debug"),
                      ]);
                    } else {
                      setLvlFilter((old) => [...old, "debug"]);
                    }
                  }}
                  checked={lvlFilter.includes("debug")}
                ></Checkbox>
                <Checkbox
                  title="Info"
                  onClick={() => {
                    if (lvlFilter.includes("info")) {
                      setLvlFilter((old) => [
                        ...old.filter((x) => x !== "info"),
                      ]);
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
                      setLvlFilter((old) => [
                        ...old.filter((x) => x !== "warn"),
                      ]);
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
                      setLvlFilter((old) => [
                        ...old.filter((x) => x !== "error"),
                      ]);
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
                color: "var(--color4)",
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
                color: "var(--color4)",
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
                color: "var(--color4)",
                height: "40px",
                alignContent: "center",
              }}
            >
              MESSAGE
            </p>
          </div>
          <Card className="min-h-[40px] bg-[#ffffff08]">
            {filteredMessages.map((x, i) => (
              <div
                className="flex p-[10px] cursor-pointer gap-[10px] hover:bg-[#ffffff10]"
                key={`${x.timestamp}_${x.msg}_${i}`}
              >
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
                  <Badge
                    variant={
                      x.lvl === "ERROR"
                        ? "destructive"
                        : x.lvl === "WARN"
                          ? "warning"
                          : "default"
                    }
                  >
                    {x.lvl}
                  </Badge>
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
                      fontSize: ".9rem",
                      alignSelf: "start",
                      overflow: "auto",
                    }}
                  >
                    <span
                      style={{ color: "var(--accent-text)", fontWeight: "bold" }}
                    >{`[${x.thread}] `}</span>

                    {x.msg}
                  </p>
                </div>
              </div>
            ))}
          </Card>
        </div>
      </CardContent>
    </Card>
  );
};

export default SiteLogs;

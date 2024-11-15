import { InProcessSiteConfig, SiteStatusEvent } from "@/generated-api";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import useWebSocket, { ReadyState } from "react-use-websocket";

export type TLogMessage = {
  msg: string;
  lvl: "INFO" | "WARN" | "ERROR" | "DEBUG" | "TRACE" ;
  thread:string
  timestamp:string
}


export type TTcpEvent = {
  Update?: {
    tcp_peer_addr: string
    connection_key: number
    client_addr: string
    target: {
      remote_target_config: any
      hosted_target_config: InProcessSiteConfig
    }
  }
  Close?:number
}

export type EventMessage = {
  Log?: TLogMessage;
  TcpEvent? : TTcpEvent,
  SiteStatusChange? : SiteStatusEvent
}

const useEventStream = () => {
  const [messageHistory, setMessageHistory] = useState<Array<TLogMessage>>([]);
  const [tcpEvents, setTcpEvents] = useState<Array<TTcpEvent>>([]);
  const queryClient = useQueryClient()
  let hostName = window.location.hostname
  if (window.location.port) {
    hostName = `${hostName}:${window.location.port}`
  }

  const baseUrl =
    import.meta.env.MODE === "development"
      ? `${import.meta.env.VITE_ODDBOX_LOGS_URL}:${import.meta.env.VITE_ODDBOX_LOGS_PORT}`
      : `wss://${hostName}`;

  const socketUrl = `${baseUrl}/ws/event_stream`;
  const { lastMessage, readyState } = useWebSocket(socketUrl);

  useEffect(() => {
    if (lastMessage !== null) {
      const msg = JSON.parse(lastMessage.data) as EventMessage;
      if (msg.Log !== undefined) {
        msg.Log.timestamp = new Date().toLocaleTimeString()
        setMessageHistory((prev) => ([
          msg.Log!,
          ...prev.slice(0,999)
        ]));
      } else if (msg.SiteStatusChange !== undefined) {
        console.log("Site status change", JSON.stringify(msg.SiteStatusChange))
        queryClient.invalidateQueries({ queryKey: ["site-status"] });
      } else if (msg.TcpEvent) {
        setTcpEvents((prev) => ([
          msg.TcpEvent!,
          ...prev.slice(0,999)
        ]));
      }
    }
  }, [lastMessage]);

  const connectionStatus = {
    [ReadyState.CONNECTING]: "Connecting",
    [ReadyState.OPEN]: "Open",
    [ReadyState.CLOSING]: "Closing",
    [ReadyState.CLOSED]: "Closed",
    [ReadyState.UNINSTANTIATED]: "Uninstantiated",
  }[readyState];

  return {
    messageHistory,
    connectionStatus,
    tcpEvents
  };
};

export default useEventStream;

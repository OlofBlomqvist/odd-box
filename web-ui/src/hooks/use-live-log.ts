import { useEffect, useState } from "react";
import useWebSocket, { ReadyState } from "react-use-websocket";

export type TLogMessage = {
  msg: string;
  lvl: "INFO" | "WARN" | "ERROR";
  thread:string
  timestamp:string
}

const useLiveLog = () => {
  const [messageHistory, setMessageHistory] = useState<Array<TLogMessage>>([]);

  let hostName = window.location.hostname
  if (window.location.port) {
    hostName = `${hostName}:${window.location.port}`
  }

  const baseUrl =
    import.meta.env.MODE === "development"
      ? `${import.meta.env.VITE_ODDBOX_LOGS_URL}:${import.meta.env.VITE_ODDBOX_LOGS_PORT}`
      : `ws://${hostName}`;

  const socketUrl = `${baseUrl}/ws/live_logs`;
  const { lastMessage, readyState } = useWebSocket(socketUrl);

  useEffect(() => {
    if (lastMessage !== null) {
      const msg = JSON.parse(lastMessage.data) as TLogMessage;
      msg.timestamp = new Date().toLocaleTimeString()
      setMessageHistory((prev) => ([
        msg,
        ...prev
      ]));
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
  };
};

export default useLiveLog;

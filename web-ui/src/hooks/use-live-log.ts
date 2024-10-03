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
  const socketUrl = "ws://localhost:1234/ws/live_logs";
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

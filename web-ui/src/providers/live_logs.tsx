import useEventStream, { TLogMessage, TTcpEvent } from "@/hooks/use-event-stream";
import { ReactNode } from "@tanstack/react-router";
import { createContext, useContext } from "react";

const LiveLogsContext = createContext<{messageHistory:TLogMessage[],tcpEvents:TTcpEvent[]}>({
    messageHistory: [],
    tcpEvents: []
});

const LiveEventStreamProvider = ({ children }: { children: ReactNode }) => {
  let { messageHistory,tcpEvents } = useEventStream();
  return (
    <LiveLogsContext.Provider value={{messageHistory, tcpEvents}}>
      {children}
    </LiveLogsContext.Provider>
  );
};

export default LiveEventStreamProvider;

export const useLiveLogsContext = () => {
    const ctx = useContext(LiveLogsContext)
    if (!ctx) {
        throw Error("Can not use context outside provider!")
    }
    return ctx
}
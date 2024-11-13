import useEventStream, { TLogMessage } from "@/hooks/use-event-stream";
import { ReactNode } from "@tanstack/react-router";
import { createContext, useContext } from "react";

const LiveLogsContext = createContext<{messageHistory:TLogMessage[]}>({
    messageHistory: []
});

const LiveEventStreamProvider = ({ children }: { children: ReactNode }) => {
  let { messageHistory } = useEventStream();
  return (
    <LiveLogsContext.Provider value={{messageHistory}}>
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
import useLiveLog, { TLogMessage } from "@/hooks/use-live-log";
import { ReactNode } from "@tanstack/react-router";
import { createContext, useContext } from "react";

const LiveLogsContext = createContext<{messageHistory:TLogMessage[]}>({
    messageHistory: []
});

const LiveLogsProvider = ({ children }: { children: ReactNode }) => {
  let { messageHistory } = useLiveLog();
  return (
    <LiveLogsContext.Provider value={{messageHistory}}>
      {children}
    </LiveLogsContext.Provider>
  );
};

export default LiveLogsProvider;

export const useLiveLogsContext = () => {
    const ctx = useContext(LiveLogsContext)
    if (!ctx) {
        throw Error("Can not use context outside provider!")
    }
    return ctx
}
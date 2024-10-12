import { ReactNode } from "@tanstack/react-router";
import {
  SetStateAction,
  createContext,
  Dispatch,
  useState,
  useContext,
} from "react";

type TDrawerContext = {
  drawerOpen: boolean;
  setDrawerOpen: Dispatch<SetStateAction<boolean>>;
};

const DrawerContext = createContext<TDrawerContext | undefined>(undefined);

const DrawerProvider = ({ children }: { children?: ReactNode }) => {
  const [drawerOpen, setDrawerOpen] = useState(false);
  return (
    <DrawerContext.Provider
      value={{
        drawerOpen,
        setDrawerOpen,
      }}
    >
      {children}
    </DrawerContext.Provider>
  );
};

export const useDrawerContext = () => {
  const ctx = useContext(DrawerContext);
  if (!ctx) {
    throw new Error("useDrawerContext must be used within a DrawerProvider");
  }
  return ctx;
};

export default DrawerProvider;

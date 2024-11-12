import { ReactNode } from "@tanstack/react-router";
import { createContext, useContext, useEffect, useState } from "react";

type TTheme = {
  lightMode: boolean;
};

const ThemeContext = createContext<TTheme>({ lightMode: true });

const ThemeContextProvider = ({ children }: { children: ReactNode }) => {
  const [lightMode, setLightMode] = useState(() => getInitialValue());

  function getInitialValue() {
    if (window.localStorage.getItem("odd-box-light-mode") === "1") {
      document.body.classList.add("light");
      return true;
    }
    return false;
  }

  useEffect(() => {
    const checkTheme = () => {
      setLightMode(document.body.classList.contains("light") ? true : false);
      window.localStorage.setItem(
        "odd-box-light-mode",
        document.body.classList.contains("light") ? "1" : "0"
      );
    };

    checkTheme();

    const observer = new MutationObserver(checkTheme);
    observer.observe(document.body, {
      attributes: true,
      attributeFilter: ["class"],
    });

    return () => observer.disconnect();
  }, []);

  return (
    <ThemeContext.Provider value={{ lightMode }}>
      {children}
    </ThemeContext.Provider>
  );
};

export const useThemeContext = () => {
  const ctx = useContext(ThemeContext);
  if (!ctx) {
    throw Error("Can not use theme context outside provider!");
  }
  return ctx;
};

export default ThemeContextProvider;

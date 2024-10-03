import { useMediaQuery } from "react-responsive";
import { useDrawerContext } from "../drawer/context";
import Hamburger from "hamburger-react";
import "./header-style.css";
import { SiteSearchBox } from "../combobox/site_search_box";
const Header = () => {
  const { setDrawerOpen, drawerOpen } = useDrawerContext();
  const isBigScreen = useMediaQuery({ query: "(min-width: 800px)" });

  return (
    <div className="odd-header">
      <div>
        {isBigScreen && (
          <img src="/webui/ob3.png" height={50} style={{ height: "50px" }} />
        )}

        {!isBigScreen && (
          <Hamburger
            size={20}
            toggled={drawerOpen}
            toggle={() => setDrawerOpen((x) => !x)}
          />
        )}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "20px" }}>
        <SiteSearchBox />

        <a
          title="GitHub"
          target="_blank"
          href="https://github.com/OlofBlomqvist/odd-box"
        >
          <img src="/webui/github2.png" style={{ height: "20px" }} />
        </a>
      </div>
    </div>
  );
};

export default Header;

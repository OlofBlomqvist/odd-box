import { useMediaQuery } from "react-responsive";
import { useDrawerContext } from "../drawer/context";
import Hamburger from "hamburger-react";
import { SiteSearchBox } from "../combobox/site_search_box";
const Header = () => {
  const { setDrawerOpen, drawerOpen } = useDrawerContext();
  const isBigScreen = useMediaQuery({ query: "(min-width: 900px)" });

  return (
    <header className="fixed flex items-center top-0 left-0 right-0 bg-[var(--bg-color)] z-[1000] h-[60px] justify-between ml:pl-[20px] pr-[20px] ml:pr-[20px]">
      <div className="flex items-center text-xl font-light">
      {isBigScreen && (
          <img src="/box2.png" height={40} style={{ height: "40px" }} />
        )}
        {!isBigScreen && (
          <Hamburger
            size={20}
            toggled={drawerOpen}
            toggle={() => setDrawerOpen((x) => !x)}
          />

        )}
        <p>odd</p><span className="text-[#ff6c00]">box</span>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "20px" }}>
        <SiteSearchBox />

        <a
          title="GitHub"
          target="_blank"
          href="https://github.com/OlofBlomqvist/odd-box"
        >
          <img src="/github2.png" style={{ height: "20px",minWidth:"20px" }} />
        </a>
      </div>
    </header>
  );
};

export default Header;

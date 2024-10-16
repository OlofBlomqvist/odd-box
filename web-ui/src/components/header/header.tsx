import { useMediaQuery } from "react-responsive";
import { useDrawerContext } from "../drawer/context";
import Hamburger from "hamburger-react";
import { SiteSearchBox } from "../combobox/site_search_box";
const Header = () => {
  const { setDrawerOpen, drawerOpen } = useDrawerContext();
  const isBigScreen = useMediaQuery({ query: "(min-width: 800px)" });

  return (
    <div className="fixed flex items-center top-0 left-0 right-0 bg-[#242424] z-[1000] h-[60px] justify-between px-2 md:px-5 lg:px-10">
      <div>
        {isBigScreen && (
          <img src="/ob3.png" height={50} style={{ height: "50px" }} />
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
          <img src="/github2.png" style={{ height: "20px" }} />
        </a>
      </div>
    </div>
  );
};

export default Header;

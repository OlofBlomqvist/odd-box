import { ReactNode, useDeferredValue, useEffect } from "react";
import Drawer from "react-modern-drawer";
import "react-modern-drawer/dist/index.css";
import { useDrawerContext } from "./context";
import "./drawer-styles.css";
import { useMediaQuery } from "react-responsive";

const SideDrawer = ({ children,bottomItem }: { bottomItem?:ReactNode,children?: ReactNode }) => {
  const { setDrawerOpen, drawerOpen } = useDrawerContext();
  const isBigScreen = useMediaQuery({ query: "(min-width: 900px)" });

  const wasBigScreen = useDeferredValue(isBigScreen);

  useEffect(() => {
    if (!isBigScreen && wasBigScreen) {
      setDrawerOpen(false);
    }
  }, [isBigScreen]);

  return (
    <>
      <Drawer
        size={!isBigScreen ? "100%" : 300}
        enableOverlay={false}
        duration={0} 
        overlayOpacity={isBigScreen ? 0 : 0.2}
        customIdSuffix="x" 
        style={{
          boxShadow: "unset",
          background: "hsl(var(--card))",
          backdropFilter: "blur(10px)",
          WebkitBackdropFilter: "blur(10px)",
          borderRight: "1px solid #242424",
          display:"flex",
          flexDirection:"column",
          justifyContent:"space-between",
          color:"hsl(var(--card-foreground))"
        }}
        onClose={() => setDrawerOpen(false)}
        open={isBigScreen || drawerOpen}
        direction={"left"}
      >

        <div style={{ padding: "0px 5px",marginTop:"60px",overflowX:"auto",paddingBottom:"50px",paddingTop:"10px" }}>{children}</div>
        {bottomItem}
      </Drawer>
    </>
  );
};

export default SideDrawer;

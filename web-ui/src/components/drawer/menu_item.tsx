import { ReactNode } from "react";
import { useDrawerContext } from "./context";
import { Link } from "@tanstack/react-router";
const MenuItem = ({
  title,
  icon,
  href,
  fontSize,
  rightIcon,
  fontWeight,
  disabled,
  rightPadding,
  onClick
}: {
  onClick?: () => void,
  rightPadding?: string;
  disabled?:boolean
  rightIcon?: ReactNode;
  isBaseRoute?: boolean;
  title: string;
  icon: ReactNode;
  href: string;
  fontSize?: string;
  fontWeight?:string
}) => {
  const { setDrawerOpen } = useDrawerContext();
const classNames = ["styled-link"];
  if (disabled) {
    classNames.push("disabled");
  }
  
  return (
    <Link disabled={disabled} resetScroll={false}
      className={classNames.join(" ")}
      onClick={() => {
        onClick?.()
        setDrawerOpen(false)
      }}  
      to={href}
      style={{
        paddingRight: rightPadding ?? "0px",
      }}
    >
      {icon}
      <span style={{fontSize:fontSize ?? '1rem', fontWeight: fontWeight ?? 'normal'}}>{title}</span>
      <span style={{marginLeft:"auto"}}>
      {rightIcon}
      </span>
    </Link>
  );
};

export default MenuItem;

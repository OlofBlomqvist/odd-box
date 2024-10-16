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

 /*  display: flex;
  color: #fff;
  text-decoration: none;
  align-items: center;
  height: 40px;
  gap: 12px;
  padding: 0px 10px;
  transition: all 0.2s;
  border-radius: 5px;
  padding-right: 0px; */

  const { setDrawerOpen } = useDrawerContext();
const classNames = ["flex items-center h-10 gap-3 px-[10px] pr-0 text-white no-underline transition-all duration-200 rounded-[5px] styled-link"];
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

import { ComponentPropsWithoutRef, ReactNode } from "react";
import { useDrawerContext } from "./context";
import { Link } from "@tanstack/react-router";

type LinkProps = ComponentPropsWithoutRef<typeof Link>

const MenuItem = ({
  title,
  icon,
  fontSize,
  rightIcon,
  fontWeight,
  disabled,
  rightPadding,
  onClick,
  to,
  searchParams,
  animateIn
}: {
  animateIn?:boolean
  searchParams?:LinkProps['search'],
  to:LinkProps['to'],
  onClick?: () => void,
  rightPadding?: string;
  disabled?:boolean
  rightIcon?: ReactNode;
  isBaseRoute?: boolean;
  title: string;
  icon: ReactNode;
  fontSize?: string;
  fontWeight?:string
}) => {


  const { setDrawerOpen } = useDrawerContext();
const classNames = ["flex items-center gap-3 px-[10px] py-[.35rem] break-all pr-0 text-[hsl(var(--card-foreground))] no-underline transition-all duration-200 rounded-[5px] styled-link"];
  if (disabled) {
    classNames.push("disabled");
  }
  if (animateIn) {
    classNames.push("animate-slideIn")
  }
  
  return ( 
    <Link disabled={disabled} resetScroll={false} activeOptions={{exact: false}}
      className={classNames.join(" ")}
      onClick={() => {
        onClick?.()
        setDrawerOpen(false)
      }}  
      to={to}
      search={searchParams}
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

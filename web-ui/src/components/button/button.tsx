import { ReactNode } from "@tanstack/react-router";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  icon?: ReactNode;
  loading?: boolean;
  dangerButton?: boolean;
  secondary?: boolean;
}

const Button = ({
  children,
  dangerButton,
  secondary,
  ...rest
}: ButtonProps) => {
  let classNames = ["styled-button"];
  if (dangerButton) {
    classNames.push("danger");
  }
  if (secondary) {
    classNames.push("secondary");
  }
  if (rest.disabled) {
    classNames.push("disabled");
  }

  if (rest.className) {
    classNames = [rest.className];
  }

  return (
    <button {...rest} className={classNames.join(" ")}>
      {children}
    </button>
  );
};

export default Button;

import { ReactNode } from "@tanstack/react-router";

const SettingsItem = ({
  title,
  subTitle,
  defaultValue,
  children,
  rowOnly,
  labelFor,
}: {
  rowOnly?: boolean;
  children?: ReactNode;
  title: string;
  subTitle?: string;
  defaultValue?: string;
  labelFor?: string;
}) => {
  const classNames = ["settings-item"];
  if (rowOnly) {
    classNames.push("row-only");
  }
  return (
    <div>
      <div className={classNames.join(" ")}>
        <div style={{ maxWidth: "400px" }}>
          <label
            htmlFor={labelFor}
            style={{ fontWeight: "bold", fontSize: ".8rem", display: "block" }}
          >
            {title}
          </label>
          <label
            htmlFor={labelFor}
            style={{ fontSize: ".8rem", opacity: 0.6, display: "block" }}
          >
            {subTitle}
            <br />
            {defaultValue && `Default: ${defaultValue}`}
          </label>
        </div>
        <div style={{ flexShrink: 0 }}>{children}</div>
      </div>
    </div>
  );
};

export default SettingsItem;

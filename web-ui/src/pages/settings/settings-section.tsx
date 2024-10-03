import { ReactNode } from "@tanstack/react-router";

const SettingsSection = ({
  children,
  noTopSeparator,
  noBottomSeparator,
  marginTop
}: {
  marginTop?: string;
  noBottomSeparator?: boolean;
  noTopSeparator?: boolean;
  children?: ReactNode;
}) => {
  return (
    <div
      className="settings-section"
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "20px",
        marginTop: marginTop ?? "20px",
        marginBottom: "20px",
      }}
    >
      {!noTopSeparator && <hr  />}
      {children}
      {!noBottomSeparator && <hr  />}
    </div>
  );
};

export default SettingsSection;

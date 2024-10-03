import { ReactNode } from "@tanstack/react-router";

const SettingsSection = ({
  children,
  noTopSeparator,
  noBottomSeparator,
}: {
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
        marginTop: "20px",
        marginBottom: "20px",
      }}
    >
      {!noTopSeparator && <hr style={{ opacity: 0.2 }} />}
      {children}
      {!noBottomSeparator && <hr style={{ opacity: 0.2 }} />}
    </div>
  );
};

export default SettingsSection;

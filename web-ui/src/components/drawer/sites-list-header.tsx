const SectionDivider = ({
  label,
  menuActions,
}: {
  label: string;
  menuActions?: React.ReactNode;
}) => {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "0px 10px",
        paddingRight: "0px",
      }}
    >
      <p className="text-[var(--color-muted)]"
        style={{
          // opacity: 0.6,
          fontSize: ".8rem",
          fontWeight: "bold",
          letterSpacing: ".12rem",
        }}
      >
        {label}
      </p>
      {menuActions}
    </div>
  );
};

export default SectionDivider;

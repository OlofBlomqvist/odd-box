
interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  withSaveButton?: boolean;
  originalValue?: string | number;
  onSave?: (newValue: string | number | readonly string[] | undefined) => void;
  disableSaveButton?:boolean
}

const Input = ({
  withSaveButton,
  originalValue,
  onSave,
  disableSaveButton,
  ...rest
}: InputProps) => {
  let textInputStyle: React.CSSProperties = {
    width: "100%",
    fontSize: ".8rem",
    minWidth: "250px",
    border: 0,
    outline: 0,
    padding: "0px 10px",
    borderRadius: withSaveButton ? 0 : "4px",
  };

  if (withSaveButton) {
    textInputStyle.borderTopRightRadius = 0;
    textInputStyle.borderBottomRightRadius = 0;
  }

  const showSaveButton = withSaveButton && originalValue !== rest.value;

  const inputSaveButtonClassNames = ["border-0 bg-[#a4dd90] outline-none w-[50px] grid place-content-center transition-all duration-100 p-0 cursor-pointer text-[#00000099] hover:bg-[#86af77]"];

  if (showSaveButton) {
    inputSaveButtonClassNames.push("border-l border-[#00000033]");
  }

  if (disableSaveButton) {
    inputSaveButtonClassNames.push("opacity-[.5] pointer-events-none")
  }

  return (
    <div
      style={{
        display: "grid",
        transition: "all .2s",
        width: "100%",
        gridTemplateColumns: showSaveButton ? "1fr 50px" : "1fr 0px",
        height: "32px",
        borderRadius: withSaveButton ? "3px" : 0,
        overflow: "hidden",
        border: rest.type === "checkbox" ? 0 : "1px solid var(--border)",
      }}
    >
      <input
        {...rest}
        className="text-black"
        style={rest.type !== "checkbox" ? textInputStyle : {}}
      />
      {rest.type !== "checkbox" && withSaveButton && (
        <button disabled={disableSaveButton}
          tabIndex={-1}
          onClick={() => onSave?.(rest.value)}
          className={inputSaveButtonClassNames.join(" ")}
        >
          <span className="text-[12px] font-bold">SAVE</span>
        </button>
      )}
    </div>
  );
};

export default Input;

import "./styles.css";

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  withSaveButton?: boolean;
  originalValue?: string | number;
  onSave?: (newValue: string | number | readonly string[] | undefined) => void;
}

const Input = ({
  withSaveButton,
  originalValue,
  onSave,
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

  const inputSaveButtonClassNames = ["input-save-button"];

  if (showSaveButton) {
    inputSaveButtonClassNames.push("show");
  }

  return (
    <div
      style={{
        display: "grid",
        transition: "all .2s",
        width: "100%",
        gridTemplateColumns: showSaveButton ? "1fr 30px" : "1fr 0px",
        height: "32px",
        borderRadius: withSaveButton ? "3px" : 0,
        overflow: "hidden",
      }}
    >
      <input
        {...rest}
        className="text-black"
        style={rest.type !== "checkbox" ? textInputStyle : {}}
      />
      {rest.type !== "checkbox" && withSaveButton && (
        <button
          tabIndex={-1}
          onClick={() => onSave?.(rest.value)}
          className={inputSaveButtonClassNames.join(" ")}
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            fill="#309030"
            width="12"
            height="12"
            viewBox="0 0 24 24"
          >
            <path d="M20.285 2l-11.285 11.567-5.286-5.011-3.714 3.716 9 8.728 15-15.285z" />
          </svg>
        </button>
      )}
    </div>
  );
};

export default Input;

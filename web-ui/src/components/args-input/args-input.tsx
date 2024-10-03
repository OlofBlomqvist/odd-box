import { useState } from "react";
import Plus2 from "../icons/plus2";
import "../key-value-input/styles.css";
import "react-responsive-modal/styles.css";
import { ArgsSheet } from "../sheet/args_sheet/args_sheet";

export type TKey = {
  value: string;
};

const ArgsInput = ({
  defaultKeys,
  onAddArg,
  onRemoveArg,
}: {
  onAddArg: (arg: string, originalValue: string | undefined) => void;
  onRemoveArg: (arg: string) => void;
  defaultKeys?: Array<string>;
}) => {
  const [modalState, setModalState] = useState<{
    show: boolean;
    value: string;
    originalValue: string | undefined;
  }>({
    show: false,
    value: "",
    originalValue: undefined,
  });
  const newVariableClicked = () => {
    setModalState({
      show: true,
      value: "",
      originalValue: "",
    });
  };

  return (
    <>
      <div
        style={{
          background: "var(--color3)",
          color: "black",
          marginTop: "10px",
          borderRadius: "5px",
          overflow: "hidden",
        }}
      >
        {defaultKeys?.map((key) => (
          <div
            key={key}
            onClick={() => {
              setModalState({
                show: true,
                value: key,
                originalValue: key,
              });
            }}
            className="env-var-item"
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              padding: "5px",
            }}
          >
            <p style={{ zIndex: 1, fontSize: ".8rem" }}>{key}</p>
          </div>
        ))}
        <div
          onClick={newVariableClicked}
          className="env-var-item"
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            padding: "5px",
          }}
        >
          <div
            style={{
              zIndex: 1,
              fontSize: ".8rem",
              display: "flex",
              alignItems: "center",
              gap: "5px",
            }}
          >
            <Plus2 />
            New argument
          </div>
        </div>
      </div>

      <ArgsSheet
        onAddArg={onAddArg}
        onRemoveArg={onRemoveArg}
        onClose={() => setModalState((old) => ({ ...old, show: false }))}
        originalValue={modalState.originalValue}
        show={modalState.show}
        value={modalState.value}
        valueChanged={(e) => setModalState((old) => ({ ...old, value: e }))}
      />
    </>
  );
};

export default ArgsInput;

import Button from "../button/button";
import { useState } from "react";
import Plus2 from "../icons/plus2";
import Input from "../input/input";
import "../key-value-input/styles.css";
import "react-responsive-modal/styles.css";
import OddModal from "../modal/modal";

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
      originalValue: undefined,
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

      <OddModal
        show={modalState.show}
        onClose={() => setModalState((old) => ({ ...old, show: false }))}
        title={modalState.originalValue ? "Edit argument" : "New argument"}
      >
        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
          <div>
            <p style={{ fontSize: ".8rem" }}>VALUE</p>
            <Input
              type="text"
              value={modalState.value}
              onChange={(e) =>
                setModalState((old) => ({ ...old, value: e.target.value }))
              }
            />
          </div>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              gap: "10px",
              marginTop: "5px",
            }}
          >
            {modalState.originalValue !== undefined && (
              <Button
                dangerButton
                onClick={() => {
                  onRemoveArg(modalState.originalValue!);
                  setModalState((old) => ({ ...old, show: false }));
                }}
              >
                Delete
              </Button>
            )}
            <Button
              secondary
              onClick={() => setModalState((old) => ({ ...old, show: false }))}
            >
              Cancel
            </Button>
            <Button
              disabled={modalState.value === ""}
              onClick={() => {
                onAddArg(modalState.value, modalState.originalValue);
                setModalState((old) => ({ ...old, show: false }));
              }}
            >
              Save
            </Button>
          </div>
        </div>
      </OddModal>
    </>
  );
};

export default ArgsInput;

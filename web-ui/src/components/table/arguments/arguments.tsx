import { ArgsSheet } from "@/components/sheet/args_sheet/args_sheet";
import {
  Table,
  TableBody,
  TableCell,
  TableFooter,
  TableRow,
} from "@/components/table/table";
import { DiamondPlus } from "lucide-react";
import { useState } from "react";

export const ArgumentsTable = ({
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

  const footerClassNames = ["hover:cursor-pointer"];

  if (defaultKeys?.length === 0) {
    footerClassNames.push("border-0");
  }

  return (
    <>
      <Table>
        <TableBody>
          {defaultKeys?.map((key) => (
            <TableRow
              key={JSON.stringify(key)}
              className="hover:cursor-pointer"
              onClick={() => {
                setModalState({
                  show: true,
                  value: key,
                  originalValue: key,
                });
              }}
            >
              <TableCell className="font-medium">{key}</TableCell>
            </TableRow>
          ))}
        </TableBody>
        <TableFooter className={footerClassNames.join(" ")}>
          <TableRow onClick={newVariableClicked}>
            <TableCell className="bg-transparent" colSpan={3}>
              <div className="flex items-center gap-2 justify-center">
                <DiamondPlus />
                <span>Add new argument</span>
              </div>
            </TableCell>
          </TableRow>
        </TableFooter>
      </Table>
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

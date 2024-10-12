import { Check } from "lucide-react";

const Checkbox = ({
  checked,
  title,
  onClick,
}: {
  title: string;
  checked: boolean;
  onClick: (value: any) => void;
}) => (
  <div className="checkbox-container" onClick={onClick}>
    <label style={{ pointerEvents: "none" }} htmlFor="use_https">
      {title}
    </label>
    <div className="border grid border-[#ffffff24] w-[18px] h-[18px] p-[2px] rounded place-content-center">
      <Check className={`${checked ? "opacity-100" : "opacity-0"}`}/>
    </div>
  </div>
);

export default Checkbox;

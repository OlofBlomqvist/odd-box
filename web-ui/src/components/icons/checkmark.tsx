// icon:273-checkmark | Icomoon https://icomoon.io/ | Keyamoon
import * as React from "react";

function CheckmarkIcon(props: React.SVGProps<SVGSVGElement>) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="currentColor"
      height="1em"
      width="1em"
      {...props}
    >
      <path fill="currentColor" d="M13.5 2L6 9.5 2.5 6 0 8.5l6 6 10-10z" />
    </svg>
  );
}

export default CheckmarkIcon;

import { ReactNode } from "react";
import Modal from "react-responsive-modal";

const OddModal = ({
  show,
  onClose,
  title,
  subtitle,
  children,
}: {
  title?: string;
  subtitle?: string;
  children?: ReactNode;
  onClose: () => void;
  show: boolean;
}) => {
  return (
    <Modal
      showCloseIcon={false}
      blockScroll={false}
      styles={{
        modal: {
          background: "transparent",
          backdropFilter: "blur(20px)",
          borderRadius: "5px",
          border: "1px solid #ffffff22",
        },
      }}
      open={show}
      onClose={onClose}
      center
    >
      <div>
        {title && <h3>{title}</h3>}
        {subtitle && <p style={{ fontSize: ".9rem" }}>{subtitle}</p>}
        {children}
      </div>
    </Modal>
  );
};

export default OddModal;

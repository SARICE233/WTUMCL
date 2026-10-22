import { BoxProps, HStack, Heading, Highlight, Image } from "@chakra-ui/react";
import styles from "@/styles/logo-title.module.css";

interface LogoTitleProps extends BoxProps {}

// A single transparent PNG serves both colour modes: SVG rendering is unreliable
// in some Windows webview scenarios, so the brand mark is served as a raster image.
export const BrandLogo: React.FC<BoxProps> = (props) => {
  return <Image src="/images/icons/Logo_128x128.png" alt="WTUMCL" {...props} />;
};

export const TitleShort: React.FC<LogoTitleProps> = (props) => {
  return (
    <Heading size="md" className={styles.title} {...props}>
      <Highlight query="L" styles={{ color: "blue.600", userSelect: "none" }}>
        WTUMCL
      </Highlight>
    </Heading>
  );
};

export const TitleFull: React.FC<LogoTitleProps> = (props) => {
  return (
    <Heading size="md" className={styles.title} {...props}>
      <Highlight query="L" styles={{ color: "blue.600", userSelect: "none" }}>
        WTUMC Launcher
      </Highlight>
    </Heading>
  );
};

export const TitleFullWithLogo: React.FC<LogoTitleProps> = (props) => {
  return (
    <HStack>
      <BrandLogo boxSize="36px" />
      <TitleFull {...props} />
    </HStack>
  );
};

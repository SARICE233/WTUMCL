import {
  BoxProps,
  HStack,
  Heading,
  Highlight,
  Image,
  useColorModeValue,
} from "@chakra-ui/react";
import styles from "@/styles/logo-title.module.css";

interface LogoTitleProps extends BoxProps {}

// Light theme uses the inverse colourway (black ring, white field) so the mark
// stays visible on light surfaces; dark theme uses the regular colourway.
export const BrandLogo: React.FC<BoxProps> = (props) => {
  const logoSrc = useColorModeValue(
    "/images/brand/WTUMCL-logo-inverse.svg",
    "/images/brand/WTUMCL-logo.svg"
  );

  return <Image src={logoSrc} alt="WTUMCL" {...props} />;
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

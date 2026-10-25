import {
  Box,
  BoxProps,
  Button,
  Stack,
  Tooltip,
  useColorModeValue,
} from "@chakra-ui/react";
import { useLayoutEffect, useRef, useState } from "react";

interface SegmentedControlProps extends BoxProps {
  size?: "2xs" | "xs" | "sm" | "md" | "lg";
  colorScheme?: string;
  items: {
    label: string | React.ReactNode;
    value: string;
    tooltip?: string;
  }[];
  selected: string;
  onSelectItem: (label: string) => void;
  direction?: "row" | "column";
  withTooltip?: boolean;
  withIndicator?: boolean;
}

interface IndicatorRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

const SegmentedControl: React.FC<SegmentedControlProps> = ({
  size = "md",
  colorScheme = "gray",
  items,
  selected,
  onSelectItem,
  direction = "row",
  withTooltip = false,
  withIndicator = false,
  ...boxProps
}) => {
  const sp = { "2xs": 0.5, xs: 0.5, sm: "0.175rem", md: 1, lg: 2 }[size];

  const palettes = useColorModeValue([100, 200, 300], [900, 800, 700]);
  const coreColor = useColorModeValue("white", "black");
  const selectedTextColor = useColorModeValue(
    `${colorScheme}.700`,
    `${colorScheme}.200`
  );

  const containerRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const [indicator, setIndicator] = useState<IndicatorRect>({
    left: 0,
    top: 0,
    width: 0,
    height: 0,
  });

  const selectedIndex = items.findIndex((item) => item.value === selected);

  // Measure the selected item after layout, so the highlight lines up with
  // labels of any length. A ResizeObserver keeps it aligned on container
  // resize (e.g. window width changes).
  useLayoutEffect(() => {
    if (!withIndicator) return;

    const measure = () => {
      const target = itemRefs.current[selectedIndex];
      if (!target) return;
      setIndicator({
        left: target.offsetLeft,
        top: target.offsetTop,
        width: target.offsetWidth,
        height: target.offsetHeight,
      });
    };

    measure();

    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver(measure);
    observer.observe(container);
    return () => observer.disconnect();
  }, [withIndicator, selectedIndex]);

  // The sliding block replaces the per-item background, so the selected item
  // stays transparent and only its text color moves with the highlight.
  const indicatorStyle: React.CSSProperties =
    direction === "column"
      ? {
          top: indicator.top,
          left: indicator.left,
          width: indicator.width,
          height: indicator.height,
          transition: "top 180ms ease, height 180ms ease",
        }
      : {
          top: indicator.top,
          left: 0,
          width: indicator.width,
          height: indicator.height,
          transform: `translateX(${indicator.left}px)`,
          transition: "transform 180ms ease, width 180ms ease",
        };

  return (
    <Box
      ref={containerRef}
      position="relative"
      bgColor={`${colorScheme}.${palettes[0]}`}
      p={sp}
      borderRadius="md"
      borderWidth="0.5px"
      borderColor={`${colorScheme}.${palettes[0]}`}
      display="inline-block"
      {...boxProps}
    >
      {withIndicator && (
        <Box
          aria-hidden="true"
          position="absolute"
          bgColor={coreColor}
          borderRadius="md"
          pointerEvents="none"
          style={indicatorStyle}
        />
      )}
      <Stack direction={direction} spacing={sp}>
        {items.map((item, index) => {
          const isSelected = selected === item.value;

          const button = (
            <Button
              key={item.value}
              ref={(element) => {
                itemRefs.current[index] = element;
              }}
              size={size == "2xs" ? "xs" : size}
              {...(size == "2xs" && { h: "1.2rem", px: 1 })}
              colorScheme={colorScheme}
              variant={isSelected && !withIndicator ? "outline" : "subtle"}
              bgColor={
                withIndicator
                  ? "transparent"
                  : isSelected
                    ? coreColor
                    : `${colorScheme}.${palettes[0]}`
              }
              color={
                withIndicator && isSelected ? selectedTextColor : undefined
              }
              style={
                withIndicator
                  ? {
                      transition:
                        "color 180ms ease, background-color 180ms ease",
                    }
                  : undefined
              }
              _hover={
                isSelected
                  ? { bgColor: withIndicator ? "transparent" : coreColor }
                  : { bg: `${colorScheme}.${palettes[1]}` }
              }
              _active={
                isSelected
                  ? { bgColor: withIndicator ? "transparent" : coreColor }
                  : { bg: `${colorScheme}.${palettes[2]}` }
              }
              onClick={() => onSelectItem(item.value)}
            >
              {item.label}
            </Button>
          );

          return (
            <Tooltip
              key={item.value}
              label={item.tooltip || item.value}
              isDisabled={!withTooltip}
            >
              {button}
            </Tooltip>
          );
        })}
      </Stack>
    </Box>
  );
};

export default SegmentedControl;

import { HStack, Skeleton, Text, VStack, useToken } from "@chakra-ui/react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import AdvancedCard from "@/components/common/advanced-card";
import { Section } from "@/components/common/section";
import SegmentedControl from "@/components/common/segmented";
import { useLauncherConfig } from "@/contexts/config";
import { useGlobalData } from "@/contexts/global-data";
import {
  DEFAULT_PLAY_TIME_WINDOW,
  PLAY_TIME_WINDOWS,
  PlayTimeWindow,
  WeeklyPlayTime,
} from "@/models/instance/misc";
import { InstanceService } from "@/services/instance";
import homeStyles from "@/styles/launch-home.module.css";

const formatDuration = (seconds: number) => {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
};

/** MM-DD of a YYYY-MM-DD date, matching `formatRelativeTime`. */
const shortDate = (date: string) => date.slice(5);

// Hand-written inline SVG chart: viewBox units per day slot. The svg is
// stretched to the card width, so strokes use vector-effect to stay even.
const CHART_HEIGHT = 44;
const CHART_DAY_UNIT = 8;
// Breathing room above the plot so the peak never touches the svg edge.
const CHART_TOP_PADDING = 6;
// Keep the baseline a few pixels above the svg floor so the area fill is not
// clipped.
const CHART_BASELINE_INSET = 3;
// Chart box and skeleton share one height, so switching the window never
// nudges the card layout.
const CHART_BOX_HEIGHT = "2.75rem";
// Value line and its skeleton share one height for the same reason.
const VALUE_HEIGHT = "2rem";

const WeeklyPlaytimeCard = () => {
  const { t } = useTranslation();
  const { config } = useLauncherConfig();
  const primaryColor = config.appearance.theme.primaryColor;
  const useLiquidGlass = config.appearance.theme.useLiquidGlassDesign;
  const { selectedPlayer, selectedInstance } = useGlobalData();

  const [windowDays, setWindowDays] = useState<PlayTimeWindow>(
    DEFAULT_PLAY_TIME_WINDOW
  );
  const [data, setData] = useState<WeeklyPlayTime>();
  const [isLoading, setIsLoading] = useState(true);
  const [hoveredIndex, setHoveredIndex] = useState<number>();
  // Remember the last hovered slot so the guide can fade out where the mouse
  // left instead of snapping back to an edge.
  const [lastHoveredIndex, setLastHoveredIndex] = useState<number>();

  const [lineColor] = useToken("colors", [`${primaryColor}.500`]);
  const [areaColor] = useToken("colors", [`${primaryColor}.200`]);

  useEffect(() => {
    let isCancelled = false;
    setIsLoading(true);
    setHoveredIndex(undefined);
    setLastHoveredIndex(undefined);

    InstanceService.getPlayTimeRange(windowDays)
      .then((response) => {
        if (!isCancelled && response.status === "success") {
          setData(response.data);
        }
      })
      .catch(() => undefined)
      .finally(() => {
        if (!isCancelled) setIsLoading(false);
      });

    return () => {
      isCancelled = true;
    };
  }, [windowDays, selectedPlayer?.id, selectedInstance?.id]);

  const days = data?.days ?? [];
  const peakSeconds = Math.max(...days.map((day) => day.seconds), 1);
  const baseline = CHART_HEIGHT - CHART_BASELINE_INSET;
  const viewWidth = Math.max(days.length, 1) * CHART_DAY_UNIT;
  const points = days.map((day, index) => ({
    x: (index + 0.5) * CHART_DAY_UNIT,
    y: baseline - (day.seconds / peakSeconds) * (baseline - CHART_TOP_PADDING),
  }));
  const linePoints = points.map((point) => `${point.x},${point.y}`).join(" ");

  // The hovered day, or today when nothing is hovered, is the one labelled.
  const focusIndex = hoveredIndex ?? days.length - 1;
  const focusDay = days[focusIndex];
  const focusLabel = focusDay
    ? [
        focusIndex === days.length - 1
          ? t("LaunchPage.weeklyPlaytime.today")
          : shortDate(focusDay.date),
        formatDuration(focusDay.seconds),
      ].join(" \u00b7 ")
    : "";

  // The guide is a translate-only group: the mouse moves it with a short
  // transition, and leaving the chart only fades it, never repositions it.
  const guideIndex = Math.min(
    lastHoveredIndex ?? days.length - 1,
    days.length - 1
  );
  const guidePoint = days.length ? points[guideIndex] : undefined;

  return (
    <Section
      title={t("LaunchPage.weeklyPlaytime.title")}
      data-launch="weekly"
      headExtra={
        <SegmentedControl
          size="2xs"
          withIndicator
          colorScheme={primaryColor}
          className={useLiquidGlass ? homeStyles.glassSegmented : undefined}
          selected={String(windowDays)}
          items={PLAY_TIME_WINDOWS.map((days) => ({
            value: String(days),
            label: t("LaunchPage.weeklyPlaytime.days", { days }),
          }))}
          onSelectItem={(value) =>
            setWindowDays(Number(value) as PlayTimeWindow)
          }
        />
      }
    >
      <AdvancedCard p={3}>
        <VStack align="stretch" spacing={2.5}>
          <HStack align="flex-start" justifyContent="space-between" spacing={3}>
            <VStack align="start" spacing={1} minW={0}>
              {isLoading ? (
                <Skeleton h={VALUE_HEIGHT} w="6rem" />
              ) : (
                <Text
                  fontSize="2xl"
                  fontWeight="bold"
                  lineHeight={VALUE_HEIGHT}
                  whiteSpace="nowrap"
                >
                  {formatDuration(data?.weekSeconds ?? 0)}
                </Text>
              )}
              <Text fontSize="xs" className="secondary-text">
                {t("LaunchPage.weeklyPlaytime.subtitle", { days: windowDays })}
              </Text>
            </VStack>
            <Text
              fontSize="xs"
              className="secondary-text"
              whiteSpace="nowrap"
              overflow="hidden"
              textOverflow="ellipsis"
              maxW="45%"
              flexShrink={0}
              textAlign="right"
              pt={2}
            >
              {focusLabel}
            </Text>
          </HStack>
          {isLoading ? (
            <Skeleton h={CHART_BOX_HEIGHT} w="100%" borderRadius="sm" />
          ) : (
            <svg
              role="img"
              aria-label={t("LaunchPage.weeklyPlaytime.subtitle", {
                days: windowDays,
              })}
              viewBox={`0 0 ${viewWidth} ${CHART_HEIGHT}`}
              style={{
                display: "block",
                width: "100%",
                height: CHART_BOX_HEIGHT,
              }}
              preserveAspectRatio="none"
              onMouseLeave={() => setHoveredIndex(undefined)}
            >
              <line
                x1={0}
                y1={baseline}
                x2={viewWidth}
                y2={baseline}
                stroke={areaColor}
                strokeWidth={1}
                vectorEffect="non-scaling-stroke"
              />
              {points.length > 1 && (
                <path
                  d={`M ${points[0].x},${baseline} L ${linePoints} L ${
                    points[points.length - 1].x
                  },${baseline} Z`}
                  fill={areaColor}
                  fillOpacity={0.45}
                  stroke="none"
                />
              )}
              <polyline
                points={linePoints}
                fill="none"
                stroke={lineColor}
                strokeWidth={1.5}
                strokeLinecap="round"
                strokeLinejoin="round"
                vectorEffect="non-scaling-stroke"
              />
              {guidePoint && (
                <g
                  style={{
                    transform: `translate(${guidePoint.x}px, 0)`,
                    transition: "transform 120ms ease-out, opacity 150ms ease",
                    opacity: hoveredIndex === undefined ? 0 : 1,
                  }}
                >
                  <line
                    x1={0}
                    y1={CHART_TOP_PADDING}
                    x2={0}
                    y2={baseline}
                    stroke={lineColor}
                    strokeWidth={1}
                    strokeDasharray="2 2"
                    strokeOpacity={0.6}
                    vectorEffect="non-scaling-stroke"
                  />
                </g>
              )}
              {days.map((day, index) => (
                <rect
                  key={day.date}
                  x={index * CHART_DAY_UNIT}
                  y={0}
                  width={CHART_DAY_UNIT}
                  height={CHART_HEIGHT}
                  fill="transparent"
                  onMouseEnter={() => {
                    setHoveredIndex(index);
                    setLastHoveredIndex(index);
                  }}
                >
                  <title>{`${day.date} \u00b7 ${formatDuration(day.seconds)}`}</title>
                </rect>
              ))}
            </svg>
          )}
          <HStack
            justifyContent="space-between"
            alignItems="center"
            spacing={2}
          >
            <Text fontSize="2xs" className="secondary-text" whiteSpace="nowrap">
              {days.length ? shortDate(days[0].date) : ""}
            </Text>
            <Text fontSize="2xs" className="secondary-text" whiteSpace="nowrap">
              {t("LaunchPage.weeklyPlaytime.today")}
            </Text>
          </HStack>
        </VStack>
      </AdvancedCard>
    </Section>
  );
};

export default WeeklyPlaytimeCard;

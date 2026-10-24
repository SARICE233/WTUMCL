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
const CHART_PADDING = 4;

const WeeklyPlaytimeCard = () => {
  const { t } = useTranslation();
  const { config } = useLauncherConfig();
  const primaryColor = config.appearance.theme.primaryColor;
  const { selectedPlayer, selectedInstance } = useGlobalData();

  const [windowDays, setWindowDays] = useState<PlayTimeWindow>(
    DEFAULT_PLAY_TIME_WINDOW
  );
  const [data, setData] = useState<WeeklyPlayTime>();
  const [isLoading, setIsLoading] = useState(true);
  const [hoveredIndex, setHoveredIndex] = useState<number>();

  const [lineColor] = useToken("colors", [`${primaryColor}.500`]);
  const [areaColor] = useToken("colors", [`${primaryColor}.200`]);

  useEffect(() => {
    let isCancelled = false;
    setIsLoading(true);
    setHoveredIndex(undefined);

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
  const baseline = CHART_HEIGHT - CHART_PADDING;
  const viewWidth = Math.max(days.length, 1) * CHART_DAY_UNIT;
  const points = days.map((day, index) => ({
    x: (index + 0.5) * CHART_DAY_UNIT,
    y: baseline - (day.seconds / peakSeconds) * (baseline - CHART_PADDING),
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

  return (
    <Section
      title={t("LaunchPage.weeklyPlaytime.title")}
      data-launch="weekly"
      headExtra={
        <SegmentedControl
          size="2xs"
          colorScheme={primaryColor}
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
        <VStack align="stretch" spacing={2}>
          <HStack align="flex-end" justifyContent="space-between" spacing={2}>
            <VStack align="start" spacing={0}>
              {isLoading ? (
                <Skeleton h="1.75rem" w="6rem" />
              ) : (
                <Text fontSize="2xl" fontWeight="bold" lineHeight="shorter">
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
              isTruncated
              flexShrink={0}
              maxW="45%"
              textAlign="right"
            >
              {focusLabel}
            </Text>
          </HStack>
          {isLoading ? (
            <Skeleton h="2.75rem" w="100%" />
          ) : (
            <svg
              role="img"
              aria-label={t("LaunchPage.weeklyPlaytime.subtitle", {
                days: windowDays,
              })}
              viewBox={`0 0 ${viewWidth} ${CHART_HEIGHT}`}
              style={{ width: "100%", height: "2.75rem" }}
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
              {focusDay && (
                <line
                  x1={points[focusIndex].x}
                  y1={CHART_PADDING}
                  x2={points[focusIndex].x}
                  y2={baseline}
                  stroke={lineColor}
                  strokeWidth={1}
                  strokeDasharray="2 2"
                  strokeOpacity={0.6}
                  vectorEffect="non-scaling-stroke"
                />
              )}
              {days.map((day, index) => (
                <rect
                  key={day.date}
                  x={index * CHART_DAY_UNIT}
                  y={0}
                  width={CHART_DAY_UNIT}
                  height={CHART_HEIGHT}
                  fill="transparent"
                  onMouseEnter={() => setHoveredIndex(index)}
                >
                  <title>{`${day.date} \u00b7 ${formatDuration(day.seconds)}`}</title>
                </rect>
              ))}
            </svg>
          )}
          <HStack justifyContent="space-between">
            <Text fontSize="2xs" className="secondary-text">
              {days.length ? shortDate(days[0].date) : ""}
            </Text>
            <Text fontSize="2xs" className="secondary-text">
              {t("LaunchPage.weeklyPlaytime.today")}
            </Text>
          </HStack>
        </VStack>
      </AdvancedCard>
    </Section>
  );
};

export default WeeklyPlaytimeCard;

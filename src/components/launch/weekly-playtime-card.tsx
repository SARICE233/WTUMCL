import { Box, HStack, Skeleton, Text, VStack } from "@chakra-ui/react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import AdvancedCard from "@/components/common/advanced-card";
import { Section } from "@/components/common/section";
import { useLauncherConfig } from "@/contexts/config";
import { useGlobalData } from "@/contexts/global-data";
import { WeeklyPlayTime } from "@/models/instance/misc";
import { InstanceService } from "@/services/instance";

const formatDuration = (seconds: number) => {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
};

const WeeklyPlaytimeCard = () => {
  const { t } = useTranslation();
  const { config } = useLauncherConfig();
  const primaryColor = config.appearance.theme.primaryColor;
  const { selectedPlayer, selectedInstance } = useGlobalData();

  const [data, setData] = useState<WeeklyPlayTime>();
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let isCancelled = false;
    setIsLoading(true);

    InstanceService.getWeeklyPlayTime()
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
  }, [selectedPlayer?.id, selectedInstance?.id]);

  const days = data?.days ?? [];
  const peakSeconds = Math.max(...days.map((day) => day.seconds), 1);
  const todayDate = days.at(-1)?.date;

  return (
    <Section title={t("LaunchPage.weeklyPlaytime.title")} data-launch="weekly">
      <AdvancedCard p={3}>
        <VStack align="stretch" spacing={2}>
          <Text fontSize="xs" className="secondary-text">
            {t("LaunchPage.weeklyPlaytime.subtitle")}
          </Text>
          {isLoading ? (
            <Skeleton h="1.75rem" w="6rem" />
          ) : (
            <Text fontSize="2xl" fontWeight="bold" lineHeight="shorter">
              {formatDuration(data?.weekSeconds ?? 0)}
            </Text>
          )}
          <HStack spacing={1} h="2.25rem" align="flex-end" aria-hidden="true">
            {Array.from({ length: 7 }).map((_, index) => {
              const day = days[index];
              const isToday = !!day && day.date === todayDate;
              return (
                <Box
                  key={day?.date ?? index}
                  flex={1}
                  minH="2px"
                  borderRadius="sm"
                  bg={isToday ? `${primaryColor}.500` : `${primaryColor}.200`}
                  h={`${Math.max(
                    ((day?.seconds ?? 0) / peakSeconds) * 100,
                    2
                  )}%`}
                  opacity={day?.seconds ? 1 : 0.45}
                />
              );
            })}
          </HStack>
        </VStack>
      </AdvancedCard>
    </Section>
  );
};

export default WeeklyPlaytimeCard;

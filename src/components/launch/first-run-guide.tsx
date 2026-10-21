import {
  Box,
  Center,
  HStack,
  Icon,
  Text,
  VStack,
  useColorModeValue,
} from "@chakra-ui/react";
import { useRouter } from "next/router";
import { useTranslation } from "react-i18next";
import { IconType } from "react-icons";
import {
  LuChevronRight,
  LuCircleUserRound,
  LuPackagePlus,
  LuRocket,
} from "react-icons/lu";
import AdvancedCard from "@/components/common/advanced-card";
import { useLauncherConfig } from "@/contexts/config";

interface GuideStep {
  key: "account" | "game" | "launch";
  icon: IconType;
  path: string;
}

const guideSteps: GuideStep[] = [
  { key: "account", icon: LuCircleUserRound, path: "/accounts?add=true" },
  { key: "game", icon: LuPackagePlus, path: "/instances/add-import" },
  { key: "launch", icon: LuRocket, path: "/instances/list" },
];

// First-run empty state: no game instance yet, so replace the recent-instance
// card with a three-step path that reuses the existing routes.
const FirstRunGuide = () => {
  const { t } = useTranslation();
  const router = useRouter();
  const { config } = useLauncherConfig();
  const primaryColor = config.appearance.theme.primaryColor;
  const stepHoverBg = useColorModeValue("blackAlpha.50", "whiteAlpha.100");

  return (
    <AdvancedCard
      data-launch="first-run-guide"
      p={{ base: 3.5, xl: 4 }}
      role="region"
      aria-label={t("LaunchPage.guide.title")}
    >
      <VStack align="stretch" spacing={1}>
        <Text fontSize={{ base: "sm", xl: "md" }} fontWeight="bold">
          {t("LaunchPage.guide.title")}
        </Text>
        <Text fontSize="xs" className="secondary-text">
          {t("LaunchPage.guide.subtitle")}
        </Text>
      </VStack>
      <VStack align="stretch" spacing={1} mt={3}>
        {guideSteps.map((step, index) => (
          <Box
            key={step.key}
            as="button"
            type="button"
            w="100%"
            p={2}
            borderRadius="md"
            textAlign="left"
            transition="background 0.16s ease"
            _hover={{ bg: stepHoverBg }}
            _focusVisible={{
              outline: "2px solid",
              outlineColor: `${primaryColor}.400`,
              outlineOffset: "1px",
            }}
            onClick={() => router.push(step.path)}
          >
            <HStack spacing={3} minW={0}>
              <Center
                boxSize="1.75rem"
                flexShrink={0}
                borderRadius="full"
                bg={`${primaryColor}.500`}
                color="white"
                fontSize="xs"
                fontWeight="bold"
              >
                {index + 1}
              </Center>
              <HStack spacing={2} flex={1} minW={0}>
                <Icon
                  as={step.icon}
                  boxSize={4}
                  color={`${primaryColor}.500`}
                  flexShrink={0}
                />
                <Box minW={0}>
                  <Text fontSize="sm" fontWeight="medium" noOfLines={1}>
                    {t(`LaunchPage.guide.${step.key}.title`)}
                  </Text>
                  <Text fontSize="xs" className="secondary-text" noOfLines={2}>
                    {t(`LaunchPage.guide.${step.key}.desc`)}
                  </Text>
                </Box>
              </HStack>
              <Icon
                as={LuChevronRight}
                boxSize={4}
                className="secondary-text"
                flexShrink={0}
              />
            </HStack>
          </Box>
        ))}
      </VStack>
    </AdvancedCard>
  );
};

export default FirstRunGuide;

import "./Logs.css";

import {
  AppShell,
  Box,
  Burger,
  Button,
  Divider,
  Group,
  NavLink,
  Table,
  Text,
  Pagination,
  Fieldset,
  Slider,
  Select,
  Stack,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { Gear, House } from "@phosphor-icons/react";
import { Link, Outlet, useParams } from "react-router-dom";

const LogViewPage = () => {
  const { id } = useParams();

  return (
    <Box>
      <Text>
        <Link to="/logs">Back</Link>
      </Text>
      <Divider my="sm" />
      <Text>Log View ({id})</Text>
    </Box>
  );
};

const LogIndexPage = () => {
  const data = [
    { id: 1, date: "2021-10-01 12:36PM", name: "Siegfried, Vane, Cagliostro, Vaseraga" },
    { id: 2, date: "2021-10-01 12:40PM", name: "Siegfried, Narmaya, Narmaya, Zeta" },
  ];

  const rows = data.map((row) => (
    <Table.Tr key={row.id}>
      <Table.Td>{row.date}</Table.Td>
      <Table.Td>{row.name}</Table.Td>
      <Table.Td>
        <Button size="xs" variant="default" component={Link} to={`/logs/${row.id}`}>
          View
        </Button>
      </Table.Td>
    </Table.Tr>
  ));

  return (
    <Box>
      <Table striped highlightOnHover>
        <Table.Thead>
          <Table.Tr>
            <Table.Th>Date</Table.Th>
            <Table.Th>Name</Table.Th>
            <Table.Th></Table.Th>
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>{rows}</Table.Tbody>
      </Table>
      <Divider my="sm" />
      <Pagination total={10} />
    </Box>
  );
};

const SettingsPage = () => {
  return (
    <Box>
      <Fieldset legend="Meter Settings">
        <Stack>
          <Text size="sm">Nothing here yet.</Text>
        </Stack>
      </Fieldset>
    </Box>
  );

  return (
    <Box>
      <Fieldset legend="Meter Settings">
        <Stack>
          <Text size="sm">Background Opacity</Text>
          <Slider defaultValue={0} label={(value) => `${value}%`} />
          <Text size="sm">Copy-to-clipboard Text Format</Text>
          <Select data={["Normal", "Compact"]} defaultValue="Normal" allowDeselect={false} />
        </Stack>
      </Fieldset>
    </Box>
  );
};

const Layout = () => {
  const [mobileOpened, { toggle: toggleMobile }] = useDisclosure();
  const [desktopOpened, { toggle: toggleDesktop }] = useDisclosure(true);

  return (
    <div className="log-window">
      <AppShell
        header={{ height: 50 }}
        navbar={{
          width: 300,
          breakpoint: "sm",
          collapsed: { mobile: !mobileOpened, desktop: !desktopOpened },
        }}
        padding="sm"
      >
        <AppShell.Header>
          <Group h="100%" px="sm">
            <Burger opened={mobileOpened} onClick={toggleMobile} hiddenFrom="sm" size="sm" />
            <Burger opened={desktopOpened} onClick={toggleDesktop} visibleFrom="sm" size="sm" />
            <Text>GBFR Logs</Text>
          </Group>
        </AppShell.Header>
        <AppShell.Navbar p="sm">
          <AppShell.Section grow>
            <NavLink label="Logs" leftSection={<House size="1rem" />} component={Link} to="/logs" />
          </AppShell.Section>
          <AppShell.Section>
            <NavLink label="Settings" leftSection={<Gear size="1rem" />} component={Link} to="/logs/settings" />
          </AppShell.Section>
        </AppShell.Navbar>
        <AppShell.Main>
          <Outlet />
        </AppShell.Main>
      </AppShell>
    </div>
  );
};

export { LogIndexPage, LogViewPage, SettingsPage };

export default Layout;

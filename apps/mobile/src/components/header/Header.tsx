import { useDrawerStatus } from '@react-navigation/drawer';
import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { MotiView } from 'moti';
import { List } from 'phosphor-react-native';
import { Pressable, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { tw, twStyle } from '~/lib/tailwind';

// Default header with search bar and button to open drawer
export default function Header() {
	const navigation = useNavigation<DrawerNavigationHelpers>();

	const { top } = useSafeAreaInsets();

	const isDrawerOpen = useDrawerStatus() === 'open';

	return (
		<View
			style={twStyle('mx-4 rounded border border-app-line bg-app-overlay', {
				marginTop: top + 10
			})}
		>
			<View style={tw`flex h-10 flex-row items-center`}>
				<Pressable
					testID="drawer-toggle"
					style={tw`h-full justify-center px-3`}
					onPress={() => navigation.openDrawer()}
				>
					<MotiView
						animate={{ rotate: isDrawerOpen ? '90deg' : '0deg' }}
						transition={{ type: 'timing' }}
					>
						<List size={20} color={tw.color('ink-faint')} weight="fill" />
					</MotiView>
				</Pressable>
				<Pressable
					style={tw`h-full flex-1 justify-center`}
					onPress={() => navigation.navigate('Search')}
				>
					<Text style={tw`text-sm font-medium text-ink-dull`}>Search</Text>
				</Pressable>
			</View>
		</View>
	);
}

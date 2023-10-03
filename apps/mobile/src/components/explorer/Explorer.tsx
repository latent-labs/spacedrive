import { useNavigation } from '@react-navigation/native';
import { FlashList } from '@shopify/flash-list';
import { Rows, SquaresFour } from 'phosphor-react-native';
import { useState } from 'react';
import { Pressable, View } from 'react-native';
import { isPath, type ExplorerItem } from '@sd/client';
import SortByMenu from '~/components/menu/SortByMenu';
import Layout from '~/constants/Layout';
import { tw } from '~/lib/tailwind';
import { type SharedScreenProps } from '~/navigation/SharedScreens';
import { getExplorerStore } from '~/stores/explorerStore';
import { useActionsModalStore } from '~/stores/modalStore';

import FileItem from './FileItem';
import FileRow from './FileRow';

type ExplorerProps = {
	items?: ExplorerItem[];
};

const Explorer = ({ items }: ExplorerProps) => {
	const navigation = useNavigation<SharedScreenProps<'Location'>['navigation']>();

	const [layoutMode, setLayoutMode] = useState<'grid' | 'list'>(getExplorerStore().layoutMode);

	function changeLayoutMode(kind: 'grid' | 'list') {
		// We need to keep layoutMode as a state to make sure flash-list re-renders.
		setLayoutMode(kind);
		getExplorerStore().layoutMode = kind;
	}

	const { modalRef, setData } = useActionsModalStore();

	function handlePress(data: ExplorerItem) {
		if (isPath(data) && data.item.is_dir && data.item.location_id !== null) {
			navigation.push('Location', {
				id: data.item.location_id,
				path: `${data.item.materialized_path}${data.item.name}/`
			});
		} else {
			setData(data);
			modalRef.current?.present();
		}
	}

	return (
		<View style={tw`flex-1`}>
			{/* Header */}
			<View style={tw`flex flex-row items-center justify-between p-3`}>
				{/* Sort By */}
				<SortByMenu />
				{/* Layout (Grid/List) */}
				{layoutMode === 'grid' ? (
					<Pressable onPress={() => changeLayoutMode('list')}>
						<SquaresFour color={tw.color('ink')} size={23} />
					</Pressable>
				) : (
					<Pressable onPress={() => changeLayoutMode('grid')}>
						<Rows color={tw.color('ink')} size={23} />
					</Pressable>
				)}
			</View>
			{/* Items */}
			{items && (
				<FlashList
					key={layoutMode}
					numColumns={layoutMode === 'grid' ? getExplorerStore().gridNumColumns : 1}
					data={items}
					keyExtractor={(item) =>
						item.type === 'NonIndexedPath' ? item.item.path : item.item.id.toString()
					}
					renderItem={({ item }) => (
						<Pressable onPress={() => handlePress(item)}>
							{layoutMode === 'grid' ? (
								<FileItem data={item} />
							) : (
								<FileRow data={item} />
							)}
						</Pressable>
					)}
					extraData={layoutMode}
					estimatedItemSize={
						layoutMode === 'grid'
							? Layout.window.width / getExplorerStore().gridNumColumns
							: getExplorerStore().listItemSize
					}
				/>
			)}
		</View>
	);
};

export default Explorer;

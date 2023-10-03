import { memo, useCallback, type HTMLAttributes, type PropsWithChildren } from 'react';
import { createSearchParams, useNavigate } from 'react-router-dom';
import {
	isPath,
	useLibraryContext,
	useLibraryMutation,
	type ExplorerItem,
	type FilePath,
	type Location,
	type NonIndexedPathItem
} from '@sd/client';
import { ContextMenu, toast } from '@sd/ui';
import { isNonEmpty } from '~/util';
import { usePlatform } from '~/util/Platform';

import { useExplorerContext } from '../Context';
import { getQuickPreviewStore } from '../QuickPreview/store';
import { uniqueId } from '../util';
import { useExplorerViewContext } from '../ViewContext';

export const useViewItemDoubleClick = () => {
	const navigate = useNavigate();
	const explorer = useExplorerContext();
	const { library } = useLibraryContext();
	const { openFilePaths } = usePlatform();

	const updateAccessTime = useLibraryMutation('files.updateAccessTime');

	const doubleClick = useCallback(
		async (item?: ExplorerItem) => {
			const selectedItems = [...explorer.selectedItems];

			if (!isNonEmpty(selectedItems)) return;

			let itemIndex = 0;
			const items = selectedItems.reduce(
				(items, selectedItem, i) => {
					const sameAsClicked = item && uniqueId(item) === uniqueId(selectedItem);

					if (sameAsClicked) itemIndex = i;

					switch (selectedItem.type) {
						case 'Location': {
							items.locations.splice(sameAsClicked ? 0 : -1, 0, selectedItem.item);
							break;
						}
						case 'NonIndexedPath': {
							items.non_indexed.splice(sameAsClicked ? 0 : -1, 0, selectedItem.item);
							break;
						}
						default: {
							for (const filePath of selectedItem.type === 'Path'
								? [selectedItem.item]
								: selectedItem.item.file_paths) {
								if (isPath(selectedItem) && selectedItem.item.is_dir) {
									items.dirs.splice(sameAsClicked ? 0 : -1, 0, filePath);
								} else {
									items.paths.splice(sameAsClicked ? 0 : -1, 0, filePath);
								}
							}
							break;
						}
					}

					return items;
				},
				{
					dirs: [],
					paths: [],
					locations: [],
					non_indexed: []
				} as {
					dirs: FilePath[];
					paths: FilePath[];
					locations: Location[];
					non_indexed: NonIndexedPathItem[];
				}
			);

			if (items.paths.length > 0) {
				if (explorer.settingsStore.openOnDoubleClick === 'openFile' && openFilePaths) {
					updateAccessTime
						.mutateAsync(items.paths.map(({ object_id }) => object_id!).filter(Boolean))
						.catch(console.error);

					try {
						await openFilePaths(
							library.uuid,
							items.paths.map(({ id }) => id)
						);
					} catch (error) {
						toast.error({ title: 'Failed to open file', body: `Error: ${error}.` });
					}
				} else if (item && explorer.settingsStore.openOnDoubleClick === 'quickPreview') {
					if (item.type !== 'Location' && !(isPath(item) && item.item.is_dir)) {
						getQuickPreviewStore().itemIndex = itemIndex;
						getQuickPreviewStore().open = true;
						return;
					}
				}
			}

			if (items.dirs.length > 0) {
				const [item] = items.dirs;
				if (item) {
					navigate({
						pathname: `../location/${item.location_id}`,
						search: createSearchParams({
							path: `${item.materialized_path}${item.name}/`
						}).toString()
					});
					return;
				}
			}

			if (items.locations.length > 0) {
				const [location] = items.locations;
				if (location) {
					navigate({
						pathname: `../location/${location.id}`,
						search: createSearchParams({
							path: `/`
						}).toString()
					});
					return;
				}
			}

			if (items.non_indexed.length > 0) {
				const [non_indexed] = items.non_indexed;
				if (non_indexed) {
					navigate({
						search: createSearchParams({ path: non_indexed.path }).toString()
					});
					return;
				}
			}
		},
		[
			explorer.selectedItems,
			explorer.settingsStore.openOnDoubleClick,
			library.uuid,
			navigate,
			openFilePaths,
			updateAccessTime
		]
	);

	return { doubleClick };
};

interface ViewItemProps extends PropsWithChildren, HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
}

export const ViewItem = memo(({ data, children, ...props }: ViewItemProps) => {
	const explorerView = useExplorerViewContext();

	const { doubleClick } = useViewItemDoubleClick();

	return (
		<ContextMenu.Root
			trigger={
				<div onDoubleClick={() => doubleClick(data)} {...props}>
					{children}
				</div>
			}
			onOpenChange={explorerView.setIsContextMenuOpen}
			disabled={explorerView.contextMenu === undefined}
			onMouseDown={(e) => e.stopPropagation()}
		>
			{explorerView.contextMenu}
		</ContextMenu.Root>
	);
});

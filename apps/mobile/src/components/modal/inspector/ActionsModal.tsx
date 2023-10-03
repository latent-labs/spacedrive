import dayjs from 'dayjs';
import {
	Copy,
	Icon,
	Info,
	LockSimple,
	LockSimpleOpen,
	Package,
	Pencil,
	Share,
	TrashSimple
} from 'phosphor-react-native';
import { PropsWithChildren, useRef } from 'react';
import { Pressable, Text, View, ViewStyle } from 'react-native';
import { byteSize, getItemFilePath, getItemObject } from '@sd/client';
import FileThumb from '~/components/explorer/FileThumb';
import FavoriteButton from '~/components/explorer/sections/FavoriteButton';
import InfoTagPills from '~/components/explorer/sections/InfoTagPills';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { tw, twStyle } from '~/lib/tailwind';
import { useActionsModalStore } from '~/stores/modalStore';

import FileInfoModal from './FileInfoModal';

type ActionsContainerProps = PropsWithChildren<{
	style?: ViewStyle;
}>;

const ActionsContainer = ({ children, style }: ActionsContainerProps) => (
	<View style={twStyle('rounded-lg bg-app-box py-3.5', style)}>{children}</View>
);

type ActionsItemProps = {
	title: string;
	icon: Icon;
	onPress?: () => void;
	isDanger?: boolean;
};

const ActionsItem = ({ icon, onPress, title, isDanger = false }: ActionsItemProps) => {
	const Icon = icon;
	return (
		<Pressable onPress={onPress} style={tw`flex flex-row items-center justify-between px-4`}>
			<Text
				style={twStyle(
					'text-base font-medium leading-none',
					isDanger ? 'text-red-600' : 'text-ink'
				)}
			>
				{title}
			</Text>
			<Icon color={isDanger ? 'red' : 'white'} size={22} />
		</Pressable>
	);
};

const ActionDivider = () => <View style={tw`my-3.5 h-[0.5px] bg-app-line/80`} />;

export const ActionsModal = () => {
	const fileInfoRef = useRef<ModalRef>(null);

	const { modalRef, data } = useActionsModalStore();

	const objectData = data && getItemObject(data);
	const filePath = data && getItemFilePath(data);

	return (
		<>
			<Modal ref={modalRef} snapPoints={['60', '90']}>
				{data && (
					<View style={tw`flex-1 px-4`}>
						<View style={tw`flex flex-row items-center`}>
							{/* Thumbnail/Icon */}
							<Pressable onPress={() => fileInfoRef.current?.present()}>
								<FileThumb data={data} size={1} />
							</Pressable>
							<View style={tw`ml-2 flex-1`}>
								{/* Name + Extension */}
								<Text
									style={tw`text-base font-bold text-gray-200`}
									numberOfLines={1}
								>
									{filePath?.name}
									{filePath?.extension && `.${filePath?.extension}`}
								</Text>
								<View style={tw`flex flex-row`}>
									<Text style={tw`text-xs text-ink-faint`}>
										{`${byteSize(filePath?.size_in_bytes_bytes)}`},
									</Text>
									<Text style={tw`text-xs text-ink-faint`}>
										{' '}
										{dayjs(filePath?.date_created).format('MMM Do YYYY')}
									</Text>
								</View>
								<InfoTagPills data={data} />
							</View>
							{objectData && <FavoriteButton style={tw`mr-4`} data={objectData} />}
						</View>
						<View style={tw`my-3`} />
						{/* Actions */}
						<ActionsContainer>
							<ActionsItem
								icon={Info}
								title="Show Info"
								onPress={() => fileInfoRef.current?.present()}
							/>
						</ActionsContainer>
						<ActionsContainer style={tw`mt-2`}>
							<ActionsItem icon={Pencil} title="Rename" />
							<ActionDivider />
							<ActionsItem icon={Copy} title="Duplicate" />
							<ActionDivider />
							<ActionsItem icon={Share} title="Share" />
						</ActionsContainer>
						<ActionsContainer style={tw`mt-2`}>
							<ActionsItem icon={LockSimple} title="Encrypt" />
							<ActionDivider />
							<ActionsItem icon={LockSimpleOpen} title="Decrypt" />
							<ActionDivider />
							<ActionsItem icon={Package} title="Compress" />
							<ActionDivider />
							<ActionsItem icon={TrashSimple} title="Delete" isDanger />
						</ActionsContainer>
					</View>
				)}
			</Modal>
			<FileInfoModal ref={fileInfoRef} data={data} />
		</>
	);
};
